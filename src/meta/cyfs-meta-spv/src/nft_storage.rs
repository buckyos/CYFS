use std::str::FromStr;
use sqlx::Row;
use cyfs_base::*;
use cyfs_base_meta::*;
use crate::{DBExecutor, get_meta_err_code, HashValueEx, MetaConnection, SPVTxStorage};

#[async_trait::async_trait]
pub trait NFTStorage {
    async fn init_nft_storage(&self) -> BuckyResult<()>;
    async fn nft_create(&self, conn: &mut MetaConnection, object_id: &ObjectId, desc: &NFTDesc, name: &str, beneficiary: &ObjectId, block_number: i64, state: &NFTState) -> BuckyResult<()>;
    async fn nft_set_name(&self, conn: &mut MetaConnection, nft_id: &ObjectId, name: &str) -> BuckyResult<()>;
    async fn nft_get(&self, object_id: &ObjectId) -> BuckyResult<NFTDetail>;
    async fn nft_get2(&self, conn: &mut MetaConnection, object_id: &ObjectId) -> BuckyResult<NFTDetail>;
    async fn nft_get_of_user(&self, user_id: &ObjectId) -> BuckyResult<Vec<NFTDetail>>;
    async fn nft_get_latest_of_user(&self, user_id: &ObjectId, block_number: i64) -> BuckyResult<Vec<NFTDetail>>;
    async fn nft_change_beneficiary(&self, conn: &mut MetaConnection, nft_id: &ObjectId, creator_id: &ObjectId, beneficiary: &ObjectId, block_number: i64, record_transfer: bool) -> BuckyResult<()>;
    async fn nft_update_state(&self, conn: &mut MetaConnection, object_id: &ObjectId, state: &NFTState) -> BuckyResult<()>;
    async fn nft_add_apply_buy(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()>;
    async fn nft_get_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>>;
    async fn nft_get_apply_buy_list(&self, nft_id: &ObjectId) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>>;
    async fn nft_remove_all_apply_buy(&self, conn: &mut MetaConnection, nft_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_remove_apply_buy(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_add_bid(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId, block_number: i64) -> BuckyResult<()>;
    async fn nft_get_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>>;
    async fn nft_get_bid_list(&self, nft_id: &ObjectId, offset: u64, length: u64) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>>;
    async fn nft_remove_all_bid(&self, conn: &mut MetaConnection, nft_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_remove_bid(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_add_like(&self, conn: &mut MetaConnection, nft_id: &ObjectId, user_id: &ObjectId, block_number: u64, create_time: u64) -> BuckyResult<()>;
    async fn nft_has_like(&self, nft_id: &ObjectId, user_id: &ObjectId) -> BuckyResult<bool>;
    async fn nft_get_latest_likes(&self, nft_id: &ObjectId, count: u64) -> BuckyResult<Vec<(ObjectId, u64, u64)>>;
    async fn nft_get_likes_count(&self, nft_id: &ObjectId) -> BuckyResult<u64>;
    async fn nft_get_latest_transfer(&self, user_id: &str, block_number: i64) -> BuckyResult<Vec<NFTTransRecord>>;
    async fn nft_get_creator_latest_transfer(&self, creator_id: &str, block_number: i64) -> BuckyResult<Vec<NFTTransRecord>>;
    async fn nft_get_price(&self, nft_id: &str) -> BuckyResult<(u64, CoinTokenId)>;
    async fn nft_update_price(&self, conn: &mut MetaConnection, nft_id: &ObjectId, price: u64, coin_id: &CoinTokenId, height: u64) -> BuckyResult<()>;
    async fn nft_get_changed_price_of_creator(&self, creator: &str, height: u64) -> BuckyResult<Vec<(String, u64, CoinTokenId, u64)>>;
}

#[async_trait::async_trait]
impl NFTStorage for SPVTxStorage {
    async fn init_nft_storage(&self) -> BuckyResult<()> {
        let mut conn = self.get_conn().await?;
        let sql = r#"create table if not exists nft (
            "object_id" char(45) PRIMARY KEY,
            "nft_label" char(45) NOT NULL,
            "desc" BLOB NOT NULL,
            "name" text NOT NULL,
            "beneficiary" char(45) NOT NULL,
            "beneficiary_get_block" integer NOT NULL,
            "state" BLOB NOT NULL,
            "like_count" integer DEFAULT 0
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"select type, name, tbl_name, sql from sqlite_master where type = "table" and name = "nft""#;
        let row = conn.query_one(sqlx::query(sql)).await?;
        let sql: String = row.get("sql");
        if sql.find("price").is_none() {
            let sql = r#"alter table nft add column "price" INTEGER default 0"#;
            conn.execute_sql(sqlx::query(sql)).await?;
            let sql = r#"alter table nft add column "coin_id" BLOB default NULL"#;
            conn.execute_sql(sqlx::query(sql)).await?;
            let sql = r#"alter table nft add column "creator" char(45) NOT NULL"#;
            conn.execute_sql(sqlx::query(sql)).await?;
            let sql = r#"alter table nft add column "price_change_height" INTEGER default 0"#;
            conn.execute_sql(sqlx::query(sql)).await?;
            let sql = r#"create index if not exists price_height_i on nft(creator, price_change_height)"#;
            conn.execute_sql(sqlx::query(sql)).await?;
        }

        let sql = r#"create unique index if not exists nft_label_index on nft(nft_label)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_beneficiary_index on nft(beneficiary, beneficiary_get_block)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create table if not exists nft_bid (
            "nft_id" char(45) not null,
            "buyer_id" char(45) not null,
            "price" integer not null,
            "coin_id" blob not null,
            "block_number" integer not null,
            PRIMARY KEY(nft_id, buyer_id)
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_bid_index on nft_bid(nft_id, block_number)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create table if not exists nft_apply_buy (
            "nft_id" char(45) not null,
            "buyer_id" char(45) not null,
            "price" integer not null,
            "coin_id" blob not null,
            PRIMARY KEY(nft_id, buyer_id)
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_apply_buy_index on nft_apply_buy(nft_id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create table if not exists nft_likes (
            "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
            "nft_id" char(45) not null,
            "user_id" char(45) not null,
            "block_number" integer not null,
            "create_time" integer not null
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_likes_index on nft_likes(nft_id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_likes_index2 on nft_likes(nft_id, user_id)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create table if not exists nft_transfers (
            "nft_id" char(45) not null,
            "from" char(45) not null,
            "to" char(45) not null,
            "block_number" integer not null
            )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_transfers_from on nft_transfers("from", block_number)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"create index if not exists nft_transfers_to on nft_transfers("to", block_number)"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let sql = r#"select type, name, tbl_name, sql from sqlite_master where type = "table" and name = "nft_transfers""#;
        let row = conn.query_one(sqlx::query(sql)).await?;
        let sql: String = row.get("sql");
        if sql.find("creator_id").is_none() {
            let sql = r#"alter table nft_transfers add column "creator_id" char(45)"#;
            conn.execute_sql(sqlx::query(sql)).await?;
            let sql = r#"create index if not exists nft_transfers_creator on nft_transfers("creator_id", block_number)"#;
            conn.execute_sql(sqlx::query(sql)).await?;
        }
        Ok(())
    }

    async fn nft_create(&self, conn: &mut MetaConnection, object_id: &ObjectId, desc: &NFTDesc, name: &str, beneficiary: &ObjectId, block_number: i64, state: &NFTState) -> BuckyResult<()> {
        let sql = "select id as num from nft_likes where nft_id = ?1";
        let rows = conn.query_all(sqlx::query(sql).bind(object_id.to_string())).await?;
        let likes_count = rows.len();

        let sql = "select object_id from nft where object_id = ?1 or nft_label = ?2";
        let ret = conn.query_one(sqlx::query(sql)
            .bind(object_id.to_string())
            .bind(desc.nft_label().to_base58())).await;
        if let Err(err) = ret {
            if ERROR_NOT_FOUND == get_meta_err_code(&err)? {
                let sql = "insert into nft (object_id, creator, nft_label, desc ,name, beneficiary, beneficiary_get_block, state, like_count, coin_id, price_change_height) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";
                conn.execute_sql(sqlx::query(sql)
                    .bind(object_id.to_string())
                    .bind(desc.owner_id().as_ref().unwrap().to_string())
                    .bind(desc.nft_label().to_base58())
                    .bind(desc.to_vec()?)
                    .bind(name)
                    .bind(beneficiary.to_string())
                    .bind(block_number)
                    .bind(state.to_vec()?)
                    .bind(likes_count as i64)
                    .bind(CoinTokenId::Coin(0).to_vec()?)
                    .bind(block_number)
                ).await?;

                let sql = r#"insert into nft_transfers (nft_id, creator_id, "from", "to", block_number) values (?1, ?2, ?3, ?4, ?5)"#;
                conn.execute_sql(
                    sqlx::query(sql)
                        .bind(object_id.to_string())
                        .bind(desc.owner_id().map_or("".to_string(), |v| v.to_string()))
                        .bind("")
                        .bind(beneficiary.to_string())
                        .bind(block_number)).await?;

                Ok(())
            } else {
                Err(err)
            }
        } else {
            Ok(())
            // Err(crate::meta_err!(ERROR_ALREADY_EXIST))
        }
    }

    async fn nft_set_name(&self, conn: &mut MetaConnection, nft_id: &ObjectId, name: &str) -> BuckyResult<()> {
        let sql = "update nft set name = ?1 where object_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(name).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_get(&self, object_id: &ObjectId) -> BuckyResult<NFTDetail> {
        let mut conn = self.get_conn().await?;
        self.nft_get2(&mut conn, object_id).await
    }

    async fn nft_get2(&self, conn: &mut MetaConnection, object_id: &ObjectId) -> BuckyResult<NFTDetail> {
        let sql = "select * from nft where object_id = ?1";
        let ret = conn.query_one(sqlx::query(sql).bind(object_id.to_string())).await?;
        Ok(NFTDetail {
            desc: NFTDesc::clone_from_slice(ret.get("desc"))?,
            name: ret.get("name"),
            beneficiary: ObjectId::from_str(ret.get("beneficiary"))?,
            state: NFTState::clone_from_slice(ret.get("state"))?,
            like_count: ret.get("like_count"),
            block_number: ret.get("beneficiary_get_block"),
            price: ret.get::<i64, _>("price") as u64,
            coin_id: CoinTokenId::clone_from_slice(ret.get("coin_id")).unwrap_or(CoinTokenId::Coin(0))
        })
    }

    async fn nft_get_of_user(&self, user_id: &ObjectId) -> BuckyResult<Vec<NFTDetail>> {
        let mut conn = self.get_conn().await?;
        let sql = "select * from nft where beneficiary = ?1 order by beneficiary_get_block desc";
        let rows = conn.query_all(sqlx::query(sql).bind(user_id.to_string())).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push(NFTDetail {
                desc: NFTDesc::clone_from_slice(row.get("desc"))?,
                name: row.get("name"),
                beneficiary: ObjectId::from_str(row.get("beneficiary"))?,
                state: NFTState::clone_from_slice(row.get("state"))?,
                like_count: row.get("like_count"),
                block_number: row.get("beneficiary_get_block"),
                price: row.get::<i64, _>("price") as u64,
                coin_id: CoinTokenId::clone_from_slice(row.get("coin_id")).unwrap_or(CoinTokenId::Coin(0))
            });
        }
        Ok(list)
    }

    async fn nft_get_latest_of_user(&self, user_id: &ObjectId, block_number: i64) -> BuckyResult<Vec<NFTDetail>> {
        let mut conn = self.get_conn().await?;
        let sql = "select * from nft where beneficiary = ?1 and beneficiary_get_block > ?2 order by beneficiary_get_block desc";
        let rows = conn.query_all(sqlx::query(sql).bind(user_id.to_string()).bind(block_number)).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push(NFTDetail {
                desc: NFTDesc::clone_from_slice(row.get("desc"))?,
                name: row.get("name"),
                beneficiary: ObjectId::from_str(row.get("beneficiary"))?,
                state: NFTState::clone_from_slice(row.get("state"))?,
                like_count: row.get("like_count"),
                block_number: row.get("beneficiary_get_block"),
                price: row.get::<i64, _>("price") as u64,
                coin_id: CoinTokenId::clone_from_slice(row.get("coin_id")).unwrap_or(CoinTokenId::Coin(0))
            });
        }
        Ok(list)
    }

    async fn nft_change_beneficiary(&self, conn: &mut MetaConnection, nft_id: &ObjectId, creator_id: &ObjectId, beneficiary: &ObjectId, block_number: i64, record_transfer: bool) -> BuckyResult<()> {
        let sql = "select beneficiary from nft where object_id = ?1";
        let row = conn.query_one(sqlx::query(sql).bind(nft_id.to_string())).await?;
        let old_beneficiary: String = row.get("beneficiary");

        let sql = "update nft set beneficiary = ?1, beneficiary_get_block = ?3 where object_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(beneficiary.to_string()).bind(nft_id.to_string()).bind(block_number)).await?;

        if record_transfer {
            let sql = r#"insert into nft_transfers (nft_id, creator_id, "from", "to", block_number) values (?1, ?2, ?3, ?4, ?5)"#;
            conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string()).bind(creator_id.to_string()).bind(old_beneficiary).bind(beneficiary.to_string()).bind(block_number)).await?;
        }

        Ok(())
    }

    async fn nft_update_state(&self, conn: &mut MetaConnection, object_id: &ObjectId, state: &NFTState) -> BuckyResult<()> {
        let sql = "update nft set state = ?1 where object_id = ?2";

        conn.execute_sql(sqlx::query(sql).bind(state.to_vec()?).bind(object_id.to_string())).await?;
        Ok(())
    }

    async fn nft_add_apply_buy(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()> {
        let sql = r#"insert into nft_apply_buy (nft_id, buyer_id, price, coin_id) values (?1, ?2, ?3, ?4)
         ON CONFLICT(nft_id, buyer_id) do update set price = ?3, coin_id = ?4"#;
        conn.execute_sql(sqlx::query(sql)
            .bind(nft_id.to_string())
            .bind(buyer_id.to_string())
            .bind(price as i64)
            .bind(coin_id.to_vec()?)).await?;
        Ok(())
    }


    async fn nft_get_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await?;
        let sql = "select * from nft_apply_buy where nft_id = ?1 and buyer_id = ?2";
        match conn.query_one(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await {
            Ok(row) => {
                Ok(Some((
                    row.get::<i64, _>("price") as u64,
                    CoinTokenId::clone_from_slice(row.get("coin_id"))?)))
            },
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }

    }

    async fn nft_get_apply_buy_list(&self, nft_id: &ObjectId) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await?;

        let sql = "select * from nft_apply_buy where nft_id = ?1";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string())).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push((
                ObjectId::from_str(row.get("buyer_id"))?,
                row.get::<i64, _>("price") as u64,
                CoinTokenId::clone_from_slice(row.get("coin_id"))?
            ));
        }
        Ok(list)
    }

    async fn nft_remove_all_apply_buy(&self, conn: &mut MetaConnection, nft_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from nft_apply_buy where nft_id = ?1";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_remove_apply_buy(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from nft_apply_buy where nft_id = ?1 and buyer_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await?;
        Ok(())
    }

    async fn nft_add_bid(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId, block_number: i64) -> BuckyResult<()> {
        let sql = r#"insert into nft_bid (nft_id, buyer_id, price, coin_id, block_number) values (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(nft_id, buyer_id) do update set price = ?3, coin_id = ?4, block_number = ?5"#;
        conn.execute_sql(sqlx::query(sql)
            .bind(nft_id.to_string())
            .bind(buyer_id.to_string())
            .bind(price as i64)
            .bind(coin_id.to_vec()?)
            .bind(block_number)).await?;
        Ok(())
    }

    async fn nft_get_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await?;
        let sql = "select * from nft_bid where nft_id = ?1 and buyer_id = ?2";
        match conn.query_one(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await {
            Ok(row) => {
                Ok(Some((
                    row.get::<i64, _>("price") as u64,
                    CoinTokenId::clone_from_slice(row.get("coin_id"))?)))
            },
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }

    }

    async fn nft_get_bid_list(&self, nft_id: &ObjectId, offset: u64, length: u64) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>> {
        let mut conn = self.get_conn().await?;

        let sql = "select * from nft_bid where nft_id = ?1 order by block_number desc limit ?2, ?3";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string()).bind(offset as i64).bind(length as i64)).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push((
                ObjectId::from_str(row.get("buyer_id"))?,
                row.get::<i64, _>("price") as u64,
                CoinTokenId::clone_from_slice(row.get("coin_id"))?
            ));
        }
        Ok(list)
    }

    async fn nft_remove_all_bid(&self, conn: &mut MetaConnection, nft_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from nft_bid where nft_id = ?1";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_remove_bid(&self, conn: &mut MetaConnection, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from nft_bid where nft_id = ?1 and buyer_id = ?2";
        conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string()).bind(buyer_id.to_string())).await?;
        Ok(())
    }

    async fn nft_add_like(&self, conn: &mut MetaConnection, nft_id: &ObjectId, user_id: &ObjectId, block_number: u64, create_time: u64) -> BuckyResult<()> {
        let sql = "select * from nft_likes where nft_id = ?1 and user_id = ?2";
        match conn.query_one(sqlx::query(sql).bind(nft_id.to_string()).bind(user_id.to_string())).await {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    let sql = "update nft set like_count = like_count + 1 where object_id = ?1";
                    conn.execute_sql(sqlx::query(sql).bind(nft_id.to_string())).await?;

                    let sql = "insert into nft_likes (nft_id, user_id, block_number, create_time) values (?1, ?2, ?3, ?4)";
                    conn.execute_sql(sqlx::query(sql)
                        .bind(nft_id.to_string())
                        .bind(user_id.to_string())
                        .bind(block_number as i64)
                        .bind(create_time as i64)).await?;
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn nft_has_like(&self, nft_id: &ObjectId, user_id: &ObjectId) -> BuckyResult<bool> {
        let sql = "select * from nft_likes where nft_id = ?1 and user_id = ?2";
        let mut conn = self.get_conn().await?;
        match conn.query_one(sqlx::query(sql).bind(nft_id.to_string()).bind(user_id.to_string())).await {
            Ok(_) => {
                Ok(true)
            }
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn nft_get_latest_likes(&self, nft_id: &ObjectId, count: u64) -> BuckyResult<Vec<(ObjectId, u64, u64)>> {
        let mut conn = self.get_conn().await?;
        let sql = "select * from nft_likes where nft_id = ?1 order by id desc limit 0, ?2";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string()).bind(count as i64)).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push((
                ObjectId::from_str(row.get("user_id"))?,
                row.get::<i64, _>("block_number") as u64,
                row.get::<i64, _>("create_time") as u64));
        }
        Ok(list)
    }

    async fn nft_get_likes_count(&self, nft_id: &ObjectId) -> BuckyResult<u64> {
        let mut conn = self.get_conn().await?;
        let sql = "select id as num from nft_likes where nft_id = ?1";
        let rows = conn.query_all(sqlx::query(sql).bind(nft_id.to_string())).await?;
        Ok(rows.len() as u64)
    }

    async fn nft_get_latest_transfer(&self, user_id: &str, block_number: i64) -> BuckyResult<Vec<NFTTransRecord>> {
        let mut conn = self.get_conn().await?;
        let sql = r#"select nft_id, "to", block_number from nft_transfers where "from" = ?1 and block_number > ?2 order by block_number desc"#;
        let rows = conn.query_all(sqlx::query(sql).bind(user_id).bind(block_number)).await?;

        let mut list = Vec::new();
        for row in rows {
            let nft_id: String = row.get("nft_id");
            let sql = "select * from nft where object_id = ?1";
            let ret = conn.query_one(sqlx::query(sql).bind(nft_id)).await?;
            list.push(NFTTransRecord {
                desc: NFTDesc::clone_from_slice(ret.get("desc"))?,
                name: ret.get("name"),
                block_number: row.get("block_number"),
                from: user_id.to_string(),
                to: row.get("to")
            });
        }

        let sql = r#"select nft_id, "from", block_number from nft_transfers where "to" = ?1 and block_number > ?2 order by block_number desc"#;
        let rows = conn.query_all(sqlx::query(sql).bind(user_id).bind(block_number)).await?;

        for row in rows {
            let nft_id: String = row.get("nft_id");
            let sql = "select * from nft where object_id = ?1";
            let ret = conn.query_one(sqlx::query(sql).bind(nft_id)).await?;
            list.push(NFTTransRecord {
                desc: NFTDesc::clone_from_slice(ret.get("desc"))?,
                name: ret.get("name"),
                block_number: row.get("block_number"),
                from: row.get("from"),
                to: user_id.to_string()
            });
        }

        Ok(list)
    }

    async fn nft_get_creator_latest_transfer(&self, creator_id: &str, block_number: i64) -> BuckyResult<Vec<NFTTransRecord>> {
        let mut conn = self.get_conn().await?;
        let sql = r#"select nft_id, "from", "to", block_number from nft_transfers where creator_id = ?1 and block_number > ?2 order by block_number desc"#;
        let rows = conn.query_all(sqlx::query(sql).bind(creator_id).bind(block_number)).await?;

        let mut list = Vec::new();
        for row in rows {
            let nft_id: String = row.get("nft_id");
            let sql = "select * from nft where object_id = ?1";
            let ret = conn.query_one(sqlx::query(sql).bind(nft_id)).await?;
            list.push(NFTTransRecord {
                desc: NFTDesc::clone_from_slice(ret.get("desc"))?,
                name: ret.get("name"),
                block_number: row.get("block_number"),
                from: row.get("from"),
                to: row.get("to")
            });
        }

        Ok(list)
    }

    async fn nft_get_price(&self, nft_id: &str) -> BuckyResult<(u64, CoinTokenId)> {
        let mut conn = self.get_conn().await?;
        let sql = r#"select price, coin_id from nft where object_id = ?"#;
        let row = conn.query_one(sqlx::query(sql).bind(nft_id)).await?;
        Ok((
            row.get::<i64, _>("price") as u64, CoinTokenId::clone_from_slice(row.get("coin_id"))?
            ))
    }

    async fn nft_update_price(&self, conn: &mut MetaConnection, nft_id: &ObjectId, price: u64, coin_id: &CoinTokenId, height: u64) -> BuckyResult<()> {
        let sql = r#"update nft set price = ?, coin_id = ?, price_change_height = ? where object_id = ?"#;
        conn.execute_sql(sqlx::query(sql).bind(price as i64).bind(coin_id.to_vec()?).bind(height as i64).bind(nft_id.to_string())).await?;
        Ok(())
    }

    async fn nft_get_changed_price_of_creator(&self, creator: &str, height: u64) -> BuckyResult<Vec<(String, u64, CoinTokenId, u64)>> {
        let sql = r#"select object_id, price, coin_id, price_change_height from nft where creator = ?1 and price_change_height > ?2"#;
        let mut conn = self.get_conn().await?;
        let rows = conn.query_all(sqlx::query(sql).bind(creator).bind(height as i64)).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push((
                row.get::<String, _>("object_id"),
                row.get::<i64, _>("price") as u64,
                CoinTokenId::clone_from_slice(row.get("coin_id")).unwrap_or(CoinTokenId::Coin(0)),
                row.get::<i64, _>("price_change_height") as u64
                ));
        }
        Ok(list)
    }
}
