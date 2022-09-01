use cyfs_base::*;
use cyfs_base_meta::*;
use sqlx::{Row};
use crate::db_helper::*;
use async_std::sync::{Mutex, MutexGuard};
use std::sync::Arc;
use crate::helper::get_meta_err_code;
use crate::db_sql::*;
use log::*;
use crate::NFTStorage;

pub struct SPVTxStorage {
    conn: Mutex<MetaConnection>,
    transaction_seq: Mutex<i32>,
}

pub type SPVTxStorageRef = Arc<SPVTxStorage>;
pub enum BlockVerifyState {
    NotVerified = 0,
    Verified,
    Invalid
}

impl BlockVerifyState {
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Option<BlockVerifyState> {
        if v == BlockVerifyState::NotVerified.to_u8() {
            return Some(BlockVerifyState::NotVerified);
        } else if v == BlockVerifyState::Verified.to_u8() {
            return Some(BlockVerifyState::Verified);
        } else if v == BlockVerifyState::Invalid.to_u8() {
            return Some(BlockVerifyState::Invalid);
        }
        None
    }
}

static INSERT_HEADER_SQL: &str = "INSERT INTO headers (hash, pre, raw, verified) VALUES(?1, ?2, ?3, ?4)";
static EXTEND_BEST_SQL: &str = "INSERT INTO best (hash, number, timestamp) VALUES(?1, ?2, ?3)";

impl SPVTxStorage {
    pub fn new(conn: MetaConnection) -> SPVTxStorageRef {
        let storage = Self {
            conn: Mutex::new(conn),
            transaction_seq: Mutex::new(0)
        };
        SPVTxStorageRef::new(storage)
    }

    // async fn get_conn(&self) -> BuckyResult<MetaConnection> {
    //     let mut options = MetaConnectionOptions::new().filename(self.db_path.as_path()).create_if_missing(true);
    //     options.log_statements(LevelFilter::Off).log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
    //     options.connect().await.map_err(map_sql_err)
    // }

    pub async fn get_conn(&self) -> BuckyResult<MutexGuard<'_, MetaConnection>> {
        Ok(self.conn.lock().await)
    }

    pub async fn being_transaction(&self) -> BuckyResult<()> {
        let mut seq = self.transaction_seq.lock().await;
        let cur_seq = *seq;
        *seq += 1;
        let pos = if cur_seq == 0 {
            None
        } else {
            Some(format!("{}", cur_seq))
        };
        let mut conn = self.get_conn().await?;
        let sql = MetaTransactionSqlCreator::begin_transaction_sql(pos);
        // println!("{}", sql.as_str());
        conn.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    pub async fn rollback(&self) -> BuckyResult<()> {
        let mut seq = self.transaction_seq.lock().await;
        let cur_seq = *seq;
        *seq -= 1;
        let pos = if cur_seq <= 1 {
            None
        } else {
            Some(format!("{}", seq))
        };
        let sql = MetaTransactionSqlCreator::rollback_transaction_sql(pos);
        // println!("{}", sql.as_str());
        self.get_conn().await?.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    pub async fn commit(&self) -> BuckyResult<()> {
        let mut seq = self.transaction_seq.lock().await;
        let cur_seq = *seq;
        *seq -= 1;
        let pos = if cur_seq == 1 {
            None
        } else {
            Some(format!("{}", seq))
        };
        let sql = MetaTransactionSqlCreator::commit_transaction_sql(pos);
        // println!("{}", sql.as_str());
        self.get_conn().await?.execute_sql(sqlx::query(sql.as_str())).await?;
        Ok(())
    }

    pub async fn init(&self) -> BuckyResult<()> {
        static INIT_TX_TBL_SQL: &str = r#"CREATE TABLE IF NOT EXISTS "tx" (
            "hash" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            "number" INTEGER NOT NULL,
            "_from" CHAR(45) NOT NULL,
            "_to" CHAR(45) NOT NULL,
            "coin_id" INTEGER,
            "value" INTEGER,
            "desc" TEXT,
            "create_time" INTEGER,
            "result" INTEGER,
            "use_fee" INTEGER,
            "nonce" INTEGER,
            "gas_coin_id" INTEGER,
            "gas_price" INTEGER,
            "max_fee" INTEGER
            )"#;
        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(INIT_TX_TBL_SQL)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS from_index ON tx (_from,number);"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS to_index ON tx (_to,number);"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        static INIT_ADRESS_TX_INDEX_SQL: &str = r#"CREATE TABLE IF NOT EXISTS "tx_index" (
            "id" INTEGER PRIMARY KEY autoincrement,
            "hash" CHAR(45) NOT NULL,
            "address" CHAR(45) NOT NULL,
            "number" INTEGER NOT NULL)"#;
        conn.execute_sql(sqlx::query(INIT_ADRESS_TX_INDEX_SQL)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS address_index ON tx_index (address,number, id);"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        static FILE_ADDRESS_SUM_AMOUNT: &str = r#"CREATE TABLE IF NOT EXISTS "file_sum_amount" (
            "address" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            "amount" INTEGER)"#;
        conn.execute_sql(sqlx::query(FILE_ADDRESS_SUM_AMOUNT)).await?;

        let sql = r#"CREATE TABLE IF NOT EXISTS "services" (
            "service_id" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
            "owner_id" CHAR(45) NOT NULL,
            "service" BLOB NOT NULL
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        if !conn.has_column("services", "owner_id").await? {
            conn.add_column("services", "owner_id CHAR(45)").await?;
        }

        let index_sql = r#"CREATE INDEX IF NOT EXISTS owner_index ON services (owner_id)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        let sql = r#"CREATE TABLE IF NOT EXISTS "contracts" (
        "contract_id" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
        "contract" BLOB NOT NULL
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        // let index_sql = r#"CREATE INDEX IF NOT EXISTS buyer_index ON contracts (service_id, buyer_id)"#;
        // conn.execute_sql(sqlx::query(index_sql)).await?;

        let sql = r#"CREATE TABLE IF NOT EXISTS "service_auths" (
            "service_id" CHAR(45) NOT NULL,
            "user_id" CHAR(45) NOT NULL,
            "contract_id" CHAR(45) NOT NULL
            )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS contract_index ON service_auths (contract_id)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        let index_sql = r#"CREATE INDEX IF NOT EXISTS service_auths_index ON service_auths (service_id, user_id)"#;
        conn.execute_sql(sqlx::query(index_sql)).await?;

        let sql = r#"CREATE TABLE IF NOT EXISTS "config" (
        "key" CHAR(45) PRIMARY KEY NOT NULL UNIQUE,
        "value" TEXT NOT NULL
        )"#;
        conn.execute_sql(sqlx::query(sql)).await?;

        static INIT_HEADER_TBL_SQL: &str = "CREATE TABLE IF NOT EXISTS \"headers\"(
            \"hash\" CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            \"pre\" CHAR(64) NOT NULL,
            \"verified\" TINYINT NOT NULL,
            \"raw\" BLOB NOT NULL);";

        static INIT_BEST_TBL_SQL: &str = "CREATE TABLE IF NOT EXISTS \"best\"(
            \"number\" INTEGER PRIMARY KEY NOT NULL UNIQUE,
            \"hash\" CHAR(64) NOT NULL,
            \"timestamp\" INTEGER NOT NULL);";
        conn.execute_sql(sqlx::query(INIT_HEADER_TBL_SQL)).await?;
        conn.execute_sql(sqlx::query(INIT_BEST_TBL_SQL)).await?;
        info!("header storage init success");

        //init table erc20_contract_tx
        for sql in INIT_ERC20_CONTRACT_TX_SQL_LIST.iter() {
            conn.execute_sql(sqlx::query(sql)).await?;
        }

        Ok(())
    }

    pub async fn config_get(&self, key: &str, default: Option<String>) -> BuckyResult<String> {
        let sql = "select value from config where key=?1";

        let ret = self.get_conn().await?.query_one(sqlx::query(sql)
            .bind(key)).await;
        match ret {
            Ok(row) => Ok(row.get("value")),
            Err(e) => {
                if get_meta_err_code(&e)? == ERROR_NOT_FOUND && default.is_some() {
                    Ok(default.unwrap())
                } else {
                    Err(crate::meta_err!(ERROR_EXCEPTION))
                }
            }
        }
    }

    pub async fn config_set(&self, key: &str, value: &str) -> BuckyResult<()> {
        let sql = "insert into config values (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2";
        self.get_conn().await?.execute_sql(sqlx::query(sql).bind(key).bind(value)).await?;
        Ok(())
    }

    pub async fn add_trans_record(&self, tx_hash: &str, height: i64, caller_id: &str, to: &str, coin_id: i16,
                                  v: i64, desc: &str, create_time: i64, result: i32, fee_used: i32, nonce: i64,
                                  gas_coin_id: i16, gas_price: i64, max_fee: i32) -> BuckyResult<()> {
        static INSERT_TX_SQL: &str = "INSERT INTO tx (hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";
        let mut conn = self.get_conn().await?;

        conn.execute_sql(sqlx::query(INSERT_TX_SQL)
            .bind(tx_hash)
            .bind(height)
            .bind(caller_id)
            .bind(to)
            .bind(coin_id)
            .bind(v)
            .bind(desc)
            .bind(create_time)
            .bind(result)
            .bind(fee_used)
            .bind(nonce)
            .bind(gas_coin_id)
            .bind(gas_price)
            .bind(max_fee)).await?;

        static INSERT_TX_INDEX_SQL: &str = "INSERT INTO tx_index (hash, address, number) VALUES (?1, ?2, ?3)";
        conn.execute_sql(sqlx::query(INSERT_TX_INDEX_SQL)
            .bind(tx_hash)
            .bind(to)
            .bind(height)).await?;

        conn.execute_sql(sqlx::query(INSERT_TX_INDEX_SQL)
            .bind(tx_hash)
            .bind(caller_id)
            .bind(height)).await?;

        Ok(())
    }

    async fn erc20_call_contract(&self, conn: &mut MutexGuard<'_, MetaConnection>, call_contract: &CallContractTx, caller_id: String, number: i64, tx_hash: String, create_time: i64, gas_price: i32, result: i32) ->BuckyResult<()> {
        // a9059cbb
        let slice = &call_contract.data[..4];
        let mut bytes = [0u8; 4];
        assert_eq!(hex::decode_to_slice("a9059cbb", &mut bytes as &mut [u8]), Ok(()));
        if slice != bytes {
            return Ok(());
        }

        // ERC20 transfer function
        let function = ethabi::Function {
            name: "transfer".to_string(),
            inputs: vec![
                ethabi::Param {name: "recipient".to_string(), kind: ethabi::ParamType::Address},
                ethabi::Param {name: "amount".to_string(), kind: ethabi::ParamType::Uint(256)}
            ],
            outputs: vec![ethabi::Param {name: "".to_string(), kind: ethabi::ParamType::Bool}],
            constant: false,
            state_mutability: ethabi::StateMutability::NonPayable
        };

        let input_map = ethabi::decode_input_from_function(&function, &call_contract.data[4..]).unwrap();

        let mut to = String::new();
        let mut amount = 0;
        for (k, v) in input_map {
            if k.trim() == "recipient" {
                to = v;
            }
            else {

                amount = v.parse::<i64>().unwrap();
            }
        }

        conn.execute_sql(sqlx::query(INSERT_CALL_CONTRACT_SQL)
        .bind(call_contract.address.to_string())
        .bind(tx_hash.as_str())
        .bind(number)
        .bind(caller_id.as_str())
        .bind(to)
        .bind(amount as i64)
        .bind(gas_price)
        .bind(create_time)
        .bind(result)).await?;

        Ok(())
    }

    pub async fn load_header_by_number(&self, n: i64) -> BuckyResult<BlockDesc> {
        static QUERY_HEADER_BY_NUMBER_SQL: &str = "SELECT raw FROM headers WHERE hash IN (SELECT hash FROM best where number=?1)";

        let mut conn = self.get_conn().await?;
        let row = conn.query_one(sqlx::query(QUERY_HEADER_BY_NUMBER_SQL).bind(n)).await?;
        let raw: Vec<u8> = row.get("raw");
        let context = NamedObjectContext::clone_from_slice(raw.as_slice())?;
        if context.obj_type() == BlockDescContentV1::obj_type() {
            let header = BlockDescV1::clone_from_slice(raw.as_slice())?;
            Ok(NamedObjectDescBuilder::new(BlockDescContent::obj_type(), BlockDescContent::V1(header)).build())
        } else {
            let header = BlockDesc::clone_from_slice(raw.as_slice())?;
            Ok(header)
        }
    }

    pub async fn load_header_by_hash(&self, hash: &BlockHash) -> BuckyResult<(BlockDesc, BlockVerifyState)> {
        static QUERY_HEADER_BY_HASH_SQL: &str = "SELECT raw, verified FROM headers WHERE hash = ?1";

        let mut conn = self.get_conn().await?;
        let row = conn.query_one(sqlx::query(QUERY_HEADER_BY_HASH_SQL).bind(hash.to_hex()?)).await?;

        let raw: Vec<u8> = row.get("raw");
        let context = NamedObjectContext::clone_from_slice(raw.as_slice())?;
        let header = if context.obj_type() == BlockDescContentV1::obj_type() {
            let header = BlockDescV1::clone_from_slice(raw.as_slice())?;
            NamedObjectDescBuilder::new(BlockDescContent::obj_type(), BlockDescContent::V1(header)).build()
        } else {
            BlockDesc::clone_from_slice(raw.as_slice())?
        };
        let verify_state = BlockVerifyState::from_u8(row.get::<i16, _>("verified") as u8).unwrap();
        Ok((header, verify_state))
    }

    pub async fn load_tip_header(&self) -> BuckyResult<BlockDesc> {
        static QUERY_TIP_HEADER_SQL: &str = "SELECT raw FROM headers WHERE hash IN (SELECT hash FROM best ORDER BY number DESC LIMIT 1)";

        let mut conn = self.get_conn().await?;
        let row = conn.query_one(sqlx::query(QUERY_TIP_HEADER_SQL)).await?;
        let raw: Vec<u8> = row.get("raw");
        let context = NamedObjectContext::clone_from_slice(raw.as_slice())?;
        if context.obj_type() == BlockDescContentV1::obj_type() {
            let header = BlockDescV1::clone_from_slice(raw.as_slice())?;
            info!("load tip header {} {}", header.number(), header.hash().to_hex().unwrap());
            Ok(NamedObjectDescBuilder::new(BlockDescContent::obj_type(), BlockDescContent::V1(header)).build())
        } else {
            let header = BlockDesc::clone_from_slice(raw.as_slice())?;
            Ok(header)
        }
    }

    pub async fn change_tip(&self, header: &BlockDesc) ->BuckyResult<()> {
        static UPDATE_VERIFY_STATE_SQL: &str = "UPDATE headers SET verified=?1 WHERE hash=?2";

        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(UPDATE_VERIFY_STATE_SQL).bind(BlockVerifyState::Verified as i16).bind(header.hash_str())).await?;
        conn.execute_sql(sqlx::query(EXTEND_BEST_SQL).bind(header.hash_str()).bind(header.number()).bind(header.create_time() as i64)).await?;
        Ok(())
    }

    pub async fn save_header(&self, header: &BlockDesc) -> BuckyResult<()> {
        let r = header.to_vec();
        if r.is_err() {
            let err = r.err().unwrap();
            error!("serialize header error: {:?}", &err);
            return Err(err);
        }
        let raw_header = r.ok().unwrap();

        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(INSERT_HEADER_SQL)
            .bind(header.hash_str())
            .bind(header.pre_block_hash_str())
            .bind(raw_header.as_slice())
            .bind(BlockVerifyState::NotVerified as i16)).await?;
        Ok(())
    }

    pub async fn add_block(&self, block: &Block) -> BuckyResult<()> {
        static INSERT_TX_SQL: &str = "INSERT INTO tx (hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";
        static INSERT_TX_INDEX_SQL: &str = "INSERT INTO tx_index (hash, address, number) VALUES (?1, ?2, ?3)";
        static INCREASE_AMOUNT: &str = "INSERT INTO file_sum_amount (address, amount) VALUES (?1, ?2) ON CONFLICT (address) DO UPDATE SET amount = amount + ?2";

        self.config_set("latest_height", block.desc().number().to_string().as_str()).await?;

        let mut conn = self.get_conn().await?;
        let transactions: &Vec<MetaTx> = block.transactions();
        let receipts: Vec<Receipt> = block.receipts();
        let event_records = block.event_records();
        let mut i = 0;
        for tx in transactions {
            let content = tx.desc().content();
            let caller_id = content.caller.id()?.to_string();
            let tx_hash = tx.desc().calculate_id().to_string();
            let receipt = receipts.get(i).unwrap();

            for body in content.body.get_obj() {
                match body {
                    MetaTxBody::TransBalance(trans) => {
                        conn.execute_sql(sqlx::query(INSERT_TX_INDEX_SQL)
                            .bind(tx_hash.clone())
                            .bind(caller_id.clone())
                            .bind(block.header().number())).await?;

                        if let CoinTokenId::Coin(coin_id) = trans.ctid {
                            for (to, v) in &trans.to {
                                conn.execute_sql(sqlx::query(INSERT_TX_SQL)
                                    .bind(tx_hash.as_str())
                                    .bind(block.header().number())
                                    .bind(caller_id.as_str())
                                    .bind(to.to_string())
                                    .bind(coin_id as i16)
                                    .bind(v)
                                    .bind("转账")
                                    .bind(tx.desc().create_time() as i64)
                                    .bind(receipt.result as i32)
                                    .bind(receipt.fee_used as i32)
                                    .bind(content.nonce)
                                    .bind(content.gas_coin_id as i16)
                                    .bind(content.gas_price as i32)
                                    .bind(content.max_fee as i32)).await?;

                                conn.execute_sql(sqlx::query(INSERT_TX_INDEX_SQL)
                                    .bind(tx_hash.as_str())
                                    .bind(to.to_string())
                                    .bind(block.header().number())).await?;

                                conn.execute_sql(sqlx::query(INSERT_TX_INDEX_SQL)
                                    .bind(tx_hash.as_str())
                                    .bind(caller_id.as_str())
                                    .bind(block.header().number())).await?;

                                // if to.obj_type_code() == ObjectTypeCode::File {
                                conn.execute_sql(sqlx::query(INCREASE_AMOUNT)
                                    .bind(to.to_string())
                                    .bind(v)).await?;
                                // }
                            }
                        }
                    }
                    MetaTxBody::WithdrawToOwner(withdraw) => {
                        if let CoinTokenId::Coin(coin_id) = withdraw.ctid {
                            conn.execute_sql(sqlx::query(INSERT_TX_SQL)
                                .bind(tx_hash.as_str())
                                .bind(block.header().number())
                                .bind(withdraw.id.to_string())
                                .bind(caller_id.as_str())
                                .bind(coin_id as i16)
                                .bind(withdraw.value)
                                .bind("提现")
                                .bind(tx.desc().create_time() as i64)
                                .bind(receipt.result as i32)
                                .bind(receipt.fee_used as i32)
                                .bind(content.nonce)
                                .bind(content.gas_coin_id as i16)
                                .bind(content.gas_price as i32)
                                .bind(content.max_fee as i32)).await?;

                            conn.execute_sql(sqlx::query(INSERT_TX_INDEX_SQL)
                                .bind(tx_hash.as_str())
                                .bind(caller_id.as_str())
                                .bind(block.header().number())).await?;
                        }
                    }
                    MetaTxBody::CallContract(call_contract) => {
                        self.erc20_call_contract(
                            &mut conn,
                            call_contract,
                            caller_id.clone(),
                            block.header().number(),
                            tx_hash.clone(),
                            tx.desc().create_time() as i64,
                            content.gas_price as i32,
                            receipt.result as i32)
                            .await?;
                    }
                    MetaTxBody::NFTCreate(nft_create_tx) => {
                        // let beneficiary = if tx.desc.author_id().is_some() {
                        //     tx.desc.author_id().as_ref().unwrap().clone()
                        // } else {
                        //     return Err(meta_err!(ERROR_PARAM_ERROR));
                        // };

                        if receipt.result == 0 {
                            let beneficiary = content.caller.id()?.clone();
                            let state = if let NFTState::Auctioning((price, coin_id, duration_block_num)) = &nft_create_tx.state {
                                NFTState::Auctioning((*price, coin_id.clone(), duration_block_num + block.desc().number() as u64))
                            } else {
                                nft_create_tx.state.clone()
                            };
                            self.nft_create(
                                &mut conn,
                                &nft_create_tx.desc.nft_id(),
                                &nft_create_tx.desc,
                                nft_create_tx.name.as_str(),
                                &beneficiary,
                                block.desc().number(),
                                &state).await?;

                            if let NFTDesc::ListDesc(sub_list) = &nft_create_tx.desc {
                                for sub_nft in sub_list.content().nft_list.iter() {
                                    let sub_id = sub_nft.calculate_id();
                                    if let NFTState::Selling((price, coin_id, stop_block)) = &nft_create_tx.state {
                                        if *price == 0 {
                                            self.nft_create(
                                                &mut conn,
                                                &sub_id,
                                                &NFTDesc::FileDesc2((sub_nft.clone(), Some(nft_create_tx.desc.nft_id()))),
                                                "",
                                                &beneficiary,
                                                block.desc().number(),
                                                &NFTState::Selling((0, coin_id.clone(), *stop_block))
                                            ).await?;
                                        } else {
                                            self.nft_create(
                                                &mut conn,
                                                &sub_id,
                                                &NFTDesc::FileDesc2((sub_nft.clone(), Some(nft_create_tx.desc.nft_id()))),
                                                "",
                                                &beneficiary,
                                                block.desc().number(),
                                                &NFTState::Selling((0, coin_id.clone(), *stop_block))
                                            ).await?;
                                        }
                                    } else {
                                        self.nft_create(
                                            &mut conn,
                                            &sub_id,
                                            &NFTDesc::FileDesc2((sub_nft.clone(), Some(nft_create_tx.desc.nft_id()))),
                                            "",
                                            &beneficiary,
                                            block.desc().number(),
                                            &NFTState::Normal
                                        ).await?;
                                    }
                                }
                            }
                        }
                    },
                    MetaTxBody::NFTCreate2(nft_create_tx) => {
                        if receipt.result == 0 {
                            let beneficiary = content.caller.id()?.clone();
                            let state = if let NFTState::Auctioning((price, coin_id, duration_block_num)) = &nft_create_tx.state {
                                NFTState::Auctioning((*price, coin_id.clone(), duration_block_num + block.desc().number() as u64))
                            } else {
                                nft_create_tx.state.clone()
                            };
                            self.nft_create(
                                &mut conn,
                                &nft_create_tx.desc.nft_id(),
                                &nft_create_tx.desc,
                                nft_create_tx.name.as_str(),
                                &beneficiary,
                                block.desc().number(),
                                &state).await?;

                            if let NFTDesc::ListDesc(sub_list) = &nft_create_tx.desc {
                                for (index, sub_nft) in sub_list.content().nft_list.iter().enumerate() {
                                    let sub_id = sub_nft.calculate_id();
                                    if let NFTState::Selling((price, coin_id, _)) = &nft_create_tx.state {
                                        if *price == 0 {
                                            self.nft_create(
                                                &mut conn,
                                                &sub_id,
                                                &NFTDesc::FileDesc2((sub_nft.clone(), Some(nft_create_tx.desc.nft_id()))),
                                                nft_create_tx.sub_names.get(index).unwrap(),
                                                &beneficiary,
                                                block.desc().number(),
                                                &nft_create_tx.sub_states.get(index).unwrap()
                                            ).await?;
                                        } else {
                                            self.nft_create(
                                                &mut conn,
                                                &sub_id,
                                                &NFTDesc::FileDesc2((sub_nft.clone(), Some(nft_create_tx.desc.nft_id()))),
                                                nft_create_tx.sub_names.get(index).unwrap(),
                                                &beneficiary,
                                                block.desc().number(),
                                                &NFTState::Selling((0, coin_id.clone(), u64::MAX))
                                            ).await?;
                                        }
                                    } else {
                                        self.nft_create(
                                            &mut conn,
                                            &sub_id,
                                            &NFTDesc::FileDesc2((sub_nft.clone(), Some(nft_create_tx.desc.nft_id()))),
                                            nft_create_tx.sub_names.get(index).unwrap(),
                                            &beneficiary,
                                            block.desc().number(),
                                            &NFTState::Normal
                                        ).await?;
                                    }
                                }
                            }
                        }
                    },
                    MetaTxBody::NFTAuction(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_update_state(&mut conn, &nft_tx.nft_id, &NFTState::Auctioning((nft_tx.price, nft_tx.coin_id.clone(), nft_tx.duration_block_num + block.desc().number() as u64))).await?;
                            self.nft_remove_all_apply_buy(&mut conn, &nft_tx.nft_id).await?;

                            let nft_detail = self.nft_get2(&mut conn, &nft_tx.nft_id).await?;
                            if let NFTDesc::ListDesc(sub_list) = &nft_detail.desc {
                                for sub_nft in sub_list.content().nft_list.iter() {
                                    let sub_id = sub_nft.calculate_id();
                                    self.nft_update_state(&mut conn, &sub_id, &NFTState::Normal).await?;
                                    self.nft_remove_all_apply_buy(&mut conn, &sub_id).await?;
                                }
                            }
                        }
                    }
                    MetaTxBody::NFTBid(nft_tx) => {
                        if receipt.result == 0 {
                            let beneficiary = content.caller.id()?;
                            self.nft_add_bid(&mut conn, &nft_tx.nft_id, &beneficiary, nft_tx.price, &nft_tx.coin_id, block.desc().number()).await?;
                        }
                    }
                    MetaTxBody::NFTBuy(nft_tx) => {
                        if receipt.result == 0 {
                            let beneficiary = content.caller.id()?.clone();
                            self.nft_update_state(&mut conn, &nft_tx.nft_id, &NFTState::Normal).await?;
                            self.nft_change_beneficiary(&mut conn, &nft_tx.nft_id, &beneficiary, block.desc().number(), true).await?;

                            let nft_detail = self.nft_get2(&mut conn, &nft_tx.nft_id).await?;
                            if let NFTDesc::ListDesc(sub_list) = &nft_detail.desc {
                                for sub_nft in sub_list.content().nft_list.iter() {
                                    let sub_id = sub_nft.calculate_id();
                                    let _ = self.nft_update_state(&mut conn, &sub_id, &NFTState::Normal).await;
                                    let _ = self.nft_change_beneficiary(&mut conn, &sub_id, &beneficiary, block.desc().number(), false).await;
                                }
                            }
                        }
                    }
                    MetaTxBody::NFTSell(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_update_state(&mut conn, &nft_tx.nft_id, &NFTState::Selling((nft_tx.price, nft_tx.coin_id.clone(), nft_tx.duration_block_num))).await?;
                            self.nft_remove_all_apply_buy(&mut conn, &nft_tx.nft_id).await?;
                        }
                    }
                    MetaTxBody::NFTSell2(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_update_state(&mut conn, &nft_tx.nft_id, &NFTState::Selling((nft_tx.price, nft_tx.coin_id.clone(), u64::MAX))).await?;
                            self.nft_remove_all_apply_buy(&mut conn, &nft_tx.nft_id).await?;

                            let nft_detail = self.nft_get2(&mut conn, &nft_tx.nft_id).await?;
                            if let NFTDesc::ListDesc(sub_list) = &nft_detail.desc {
                                if nft_tx.price == 0 {
                                    for (index, sub_nft) in sub_list.content().nft_list.iter().enumerate() {
                                        let sub_id = sub_nft.calculate_id();
                                        let (coin_id, price) = nft_tx.sub_sell_infos.get(index).unwrap();
                                        self.nft_update_state(&mut conn, &sub_id, &NFTState::Selling((*price, coin_id.clone(), u64::MAX))).await?;
                                        self.nft_remove_all_apply_buy(&mut conn, &sub_id).await?;
                                    }
                                } else {
                                    for sub_nft in sub_list.content().nft_list.iter() {
                                        let sub_id = sub_nft.calculate_id();
                                        self.nft_update_state(&mut conn, &sub_id, &NFTState::Selling((0, nft_tx.coin_id.clone(), u64::MAX))).await?;
                                        self.nft_remove_all_apply_buy(&mut conn, &sub_id).await?;
                                    }
                                }
                            }
                        }
                    }
                    MetaTxBody::NFTApplyBuy(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_add_apply_buy(&mut conn, &nft_tx.nft_id, &content.caller.id()?, nft_tx.price, &nft_tx.coin_id).await?;
                        }
                    }
                    MetaTxBody::NFTCancelApplyBuyTx(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_remove_apply_buy(&mut conn, &nft_tx.nft_id, &content.caller.id()?).await?;
                        }
                    }
                    MetaTxBody::NFTAgreeApply(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_update_state(&mut conn, &nft_tx.nft_id, &NFTState::Normal).await?;
                            self.nft_change_beneficiary(&mut conn, &nft_tx.nft_id, &nft_tx.user_id, block.desc().number(), true).await?;

                            let nft_detail = self.nft_get2(&mut conn, &nft_tx.nft_id).await?;
                            if let NFTDesc::ListDesc(sub_list) = &nft_detail.desc {
                                for sub_nft in sub_list.content().nft_list.iter() {
                                    let sub_id = sub_nft.calculate_id();
                                    self.nft_update_state(&mut conn, &sub_id, &NFTState::Normal).await?;
                                    self.nft_change_beneficiary(&mut conn, &sub_id, &nft_tx.user_id, block.desc().number(), false).await?;
                                }
                            }
                        }
                    }
                    MetaTxBody::NFTLike(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_add_like(&mut conn, &nft_tx.nft_id, &content.caller.id()?, block.desc().number() as u64, tx.desc().create_time()).await?;
                        }
                    }
                    MetaTxBody::NFTSetNameTx(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_set_name(&mut conn, &nft_tx.nft_id, nft_tx.name.as_str()).await?;
                        }
                    }
                    MetaTxBody::NFTCancelSellTx(nft_tx) => {
                        if receipt.result == 0 {
                            self.nft_update_state(&mut conn, &nft_tx.nft_id, &NFTState::Normal).await?;
                        }
                    }
                    // MetaTxBody::SNService(sn_service) => {
                    //     match sn_service {
                    //         SNServiceTx::Publish(service) => {
                    //             self.add_service(service).await?;
                    //         }
                    //         SNServiceTx::Purchase(contract) => {
                    //             self.add_contract(contract).await?;
                    //         }
                    //         SNServiceTx::Remove(service_id) => {
                    //             self.delete_service(service_id).await?;
                    //         }
                    //         _ => {}
                    //     }
                    // }
                    _ => {}
                }
            }
            i += 1;
        }
        for event in event_records.iter() {
            match &event.event {
                Event::Rent(_) => {}
                Event::NameRent(_) => {}
                Event::ChangeNameEvent(_) => {}
                Event::BidName(_) => {}
                Event::StopAuction(_) => {}
                Event::UnionWithdraw(_) => {}
                Event::Extension(_) => {}
                Event::NFTStopAuction(params) => {
                    if event.event_result.status == 0 {
                        if event.event_result.data.len() > 0 {
                            let beneficiary = ObjectId::clone_from_slice(event.event_result.data.as_slice());
                            self.nft_remove_all_bid(&mut conn, &params.nft_id).await?;
                            self.nft_change_beneficiary(&mut conn, &params.nft_id, &beneficiary, block.desc().number(), true).await?;

                            let nft_detail = self.nft_get2(&mut conn, &params.nft_id).await?;
                            if let NFTDesc::ListDesc(sub_list) = &nft_detail.desc {
                                for sub_nft in sub_list.content().nft_list.iter() {
                                    let sub_id = sub_nft.calculate_id();
                                    self.nft_change_beneficiary(&mut conn, &sub_id, &beneficiary, block.desc().number(), false).await?;
                                }
                            }
                        }
                        self.nft_update_state(&mut conn, &params.nft_id, &NFTState::Normal).await?;
                    }
                }
                Event::NFTCancelApplyBuy(_) => {}
                Event::NFTStopSell(params) => {
                    if event.event_result.status == 0 {
                        self.nft_update_state(&mut conn, &params.nft_id, &NFTState::Normal).await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn row_to_obj(&self, rows: Vec<<MetaDatabase as sqlx::Database>::Row>) -> BuckyResult<Vec<SPVTx>> {
        let mut tx_list = Vec::new();
        for row in rows {
            let create_time: i64 = row.get("create_time");
            tx_list.push(SPVTx {
                hash: row.get("hash"),
                number: row.get("number"),
                from: row.get("_from"),
                to: row.get("_to"),
                coin_id: row.get::<i16, _>("coin_id") as u8,
                value: row.get("value"),
                desc: row.get("desc"),
                create_time: bucky_time_to_js_time(create_time as u64) as i64,
                result: row.get::<i32, _>("result") as u32,
                use_fee: row.get::<i32, _>("use_fee") as u32,
                nonce: row.get("nonce"),
                gas_coin_id: row.get::<i16, _>("gas_coin_id") as u8,
                gas_price: row.get::<i32, _>("gas_price") as u16,
                max_fee: row.get::<i32, _>("max_fee") as u32
            })
        }
        Ok(tx_list)
    }

    pub async fn get_payment_tx_list(&self, address_list: Vec<String>, block_section: Option<(i64, i64)>, offset: i64, length: i64, coin_id_list: Vec<String>) -> BuckyResult<Vec<SPVTx>> {
        let mut conn = self.get_conn().await?;
        let addresses = r#"""#.to_owned() + address_list.join(r#"",""#).as_str() + r#"""#;
        let coin_ids = r#"""#.to_owned() + coin_id_list.join(r#"",""#).as_str() + r#"""#;
        if block_section.is_some() {
            let (start_block, end_block) = block_section.unwrap();
            let sql = format!(r#"select hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee from tx where _from in ({}) and number >= ?1 and number <= ?2 and coin_id in ({})  order by number DESC limit ?3, ?4"#, addresses, coin_ids);

            self.row_to_obj(conn.query_all(sqlx::query(sql.as_str())
                .bind(start_block)
                .bind(end_block)
                .bind(offset)
                .bind(length)).await?).await
        } else {
            let sql = format!(r#"select hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee from tx where _from in ({}) and coin_id in ({}) order by number DESC limit ?1, ?2"#, addresses, coin_ids);
            self.row_to_obj(conn.query_all(sqlx::query(sql.as_str())
                .bind(offset)
                .bind(length)).await?).await
        }
    }

    pub async fn get_collect_tx_list(&self, address_list: Vec<String>, block_section: Option<(i64, i64)>, offset: i64, length: i64, coin_id_list: Vec<String>) -> BuckyResult<Vec<SPVTx>> {
        let mut conn = self.get_conn().await?;
        let addresses = r#"""#.to_owned() + address_list.join(r#"",""#).as_str() + r#"""#;
        let coin_ids = r#"""#.to_owned() + coin_id_list.join(r#"",""#).as_str() + r#"""#;
        if block_section.is_some() {
            let (start_block, end_block) = block_section.unwrap();
            let sql = format!(r#"select hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee from tx where _to in ({}) and number >= ?1 and number <= ?2 and coin_id in ({}) order by number DESC limit ?3, ?4"#, addresses, coin_ids);
            self.row_to_obj(conn.query_all(sqlx::query(sql.as_str())
                .bind(start_block)
                .bind(end_block)
                .bind(offset)
                .bind(length)).await?).await
        } else {
            let sql = format!(r#"select hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee from tx where _to in ({}) and coin_id in ({})  order by number DESC limit ?1, ?2"#, addresses, coin_ids);
            self.row_to_obj(conn.query_all(sqlx::query(sql.as_str())
                .bind(offset)
                .bind(length)).await?).await
        }
    }

    pub async fn get_tx_list(&self, address_list: Vec<String>, block_section: Option<(i64, i64)>, offset: i64, length: i64,  coin_id_list: Vec<String>) -> BuckyResult<Vec<SPVTx>> {
        let mut conn = self.get_conn().await?;
        let addresses = r#"""#.to_owned() + address_list.join(r#"",""#).as_str() + r#"""#;
        let coin_ids = r#"""#.to_owned() + coin_id_list.join(r#"",""#).as_str() + r#"""#;

        if block_section.is_some() {
            let (start_block, end_block) = block_section.unwrap();
            let sql = format!(r#"select hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee from tx where hash in (select hash from tx_index where address in ({}) and number >= ?1 and number <= ?2  order by number DESC limit ?3, ?4) and coin_id in ({}) order by number DESC"#, addresses, coin_ids);
            self.row_to_obj(conn.query_all(sqlx::query(sql.as_str())
                .bind(start_block)
                .bind(end_block)
                .bind(offset)
                .bind(length)).await?).await

        } else {
            let sql = format!(r#"select hash, number, _from, _to, coin_id, value, desc, create_time, result, use_fee, nonce, gas_coin_id, gas_price, max_fee from tx where hash in (select hash from tx_index where address in ({}) order by number DESC limit ?1, ?2)  and coin_id in ({}) order by number DESC"#, addresses, coin_ids);
            self.row_to_obj(conn.query_all(sqlx::query(sql.as_str())
                .bind(offset)
                .bind(length)).await?).await
        }
    }

    pub async fn get_file_amount(&self, address: String) -> BuckyResult<i64> {
        let sql = r#"select amount from file_sum_amount where address = ?1"#;

        let mut conn = self.get_conn().await?;
        let ret = conn.query_one(sqlx::query(sql).bind(address)).await;

        if ret.is_err() {
            return Ok(0);
        } else {
            Ok(ret.unwrap().get("amount"))
        }
    }

    async fn add_service(&self, service: &SNService) -> BuckyResult<()> {
        let sql = "insert into services (service_id, owner_id, service) values (?1, ?2, ?3)";
        let service_id = service.desc().calculate_id();
        let owner = {
            let owner = service.desc().owner();
            if owner.is_some() {
                owner.unwrap().to_string()
            } else {
                "".to_owned()
            }
        };
        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(sql)
            .bind(service_id.to_string())
            .bind(owner)
            .bind(service.to_vec()?)).await?;
        Ok(())
    }

    async fn update_service(&self, service: &SNService) -> BuckyResult<()> {
        let sql = "update services set service = 1? where service_id = ?2";
        let service_id = service.desc().calculate_id();
        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(sql)
            .bind(service.to_vec()?)
            .bind(service_id.to_string())).await?;
        Ok(())
    }

    async fn delete_service(&self, service_id: &ObjectId) -> BuckyResult<()> {
        let sql = "delete from services where service_id = ?1";
        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(sql).bind(service_id.to_string())).await?;
        Ok(())
    }

    pub async fn get_service(&self, service_id: &str) -> BuckyResult<SNService> {
        let sql = "select * from services where service_id = ?1";
        let mut conn = self.get_conn().await?;
        let row = conn.query_one(sqlx::query(sql).bind(service_id.to_string())).await?;
        let service = SNService::clone_from_slice(row.get::<Vec<u8>, &str>("service").as_slice())?;
        Ok(service)
    }

    async fn add_contract(&self, contract: &Contract) -> BuckyResult<()> {
        let sql = "insert into contracts (contract_id, contract) values (?1, ?2)";
        let contract_id = contract.desc().calculate_id();
        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(sql)
            .bind(contract_id.to_string())
            .bind(contract.to_vec()?)).await?;
        Ok(())
    }

    async fn update_contract(&self, contract: &Contract) -> BuckyResult<()> {
        let sql = "update contracts set contract=1? where contract_id = ?2";
        let contract_id = contract.desc().calculate_id();
        let mut conn = self.get_conn().await?;
        conn.execute_sql(sqlx::query(sql)
            .bind(contract.to_vec()?)
            .bind(contract_id.to_string())).await?;
        Ok(())
    }

    pub async fn get_contract(&self, contract_id: &str) -> BuckyResult<Contract> {
        let sql = "select * from contract_id = ?1";
        let mut conn = self.get_conn().await?;
        let row = conn.query_one(sqlx::query(sql)
            .bind(contract_id)).await?;
        let contract: Vec<u8> = row.get("contract");

        Ok(Contract::clone_from_slice(contract.as_slice())?)
    }

    pub async fn get_auth_contract(&self, service_id: &str, user_id: &str) -> BuckyResult<Contract> {
        let sql = "select contract_id from contract_auths where service_id = ?1 and user_id = ?2";
        let mut conn = self.get_conn().await?;
        let row = conn.query_one(sqlx::query(sql)
            .bind(service_id)
            .bind(user_id)).await?;

        self.get_contract(row.get("contract_id")).await
    }

    pub async fn get_status(&self) -> BuckyResult<ChainStatus> {
        let block_header = self.load_tip_header().await?;
        Ok(ChainStatus {
            version: 0,
            height: block_header.number(),
            gas_price: GasPrice {
                low: 0,
                medium: 0,
                high: 0
            }
        })
    }

    pub async fn get_erc20_contract_tx(&self, address: &str, tx_hash: &str, start_block: i64, end_block: i64, from_str: &str, to_str: &str) -> BuckyResult<Vec<ERC20ContractTxResponse>> {

        let mut conn = self.get_conn().await?;
        let mut list = Vec::new();

        if tx_hash.len() > 0 {
            let sql = r#"select address, hash, _from, _to, value, number, gas_price, created_time, result from erc20_contract_tx where hash = ?1 order by number DESC"#;
            let rows = conn.query_all(sqlx::query(sql)
                .bind(tx_hash)).await?;

            for row in rows {
                let create_time: i64 = row.get("created_time");
                list.push(ERC20ContractTxResponse {
                    address: row.get("address"),
                    tx_hash: row.get("hash"),
                    value: row.get::<i64, _>("value") as u64,
                    from: row.get("_from"),
                    to: row.get("_to"),
                    height: row.get::<i64, _>("number") as u64,
                    gas_price: row.get::<i64, _>("gas_price") as u64,
                    create_time: bucky_time_to_js_time(create_time as u64),
                    result: row.get::<i32, _>("result"),
                })

            }

            return Ok(list);
        }

        if from_str.len() == 0
            && to_str.len() == 0 {
            let sql = r#"select address, hash, _from, _to, value, number, gas_price, created_time, result from erc20_contract_tx where address = ?1 and number between ?2 and ?3 order by number DESC"#;
            let rows = conn.query_all(sqlx::query(sql)
                .bind(address.to_string())
                .bind(start_block as i64)
                .bind(end_block as i64)).await?;

            for row in rows {
                let create_time: i64 = row.get("created_time");
                list.push(ERC20ContractTxResponse {
                    address: row.get("address"),
                    tx_hash: row.get("hash"),
                    value: row.get::<i64, _>("value") as u64,
                    from: row.get("_from"),
                    to: row.get("_to"),
                    height: row.get::<i64, _>("number") as u64,
                    gas_price: row.get::<i64, _>("gas_price") as u64,
                    create_time: bucky_time_to_js_time(create_time as u64),
                    result: row.get::<i32, _>("result"),
                })

            }
        }

        if from_str.len() > 0
        && to_str.len() == 0 {
        let sql = r#"select address, hash, _from, _to, value, number, gas_price, created_time, result from erc20_contract_tx where address = ?1 and number between ?2 and ?3 and _from = ?4 order by number DESC"#;
        let rows = conn.query_all(sqlx::query(sql)
            .bind(address.to_string())
            .bind(start_block as i64)
            .bind(end_block as i64)
            .bind(from_str)).await?;

        for row in rows {
            let create_time: i64 = row.get("created_time");
            list.push(ERC20ContractTxResponse {
                address: row.get("address"),
                tx_hash: row.get("hash"),
                value: row.get::<i64, _>("value") as u64,
                from: row.get("_from"),
                to: row.get("_to"),
                height: row.get::<i64, _>("number") as u64,
                gas_price: row.get::<i64, _>("gas_price") as u64,
                create_time: bucky_time_to_js_time(create_time as u64),
                result: row.get::<i32, _>("result"),
                })

            }
        }

        if from_str.len() == 0
        && to_str.len() > 0 {
        let sql = r#"select address, hash, _from, _to, value, number, gas_price, created_time, result from erc20_contract_tx where address = ?1 and number between ?2 and ?3 and _to = ?4 order by number DESC"#;
        let rows = conn.query_all(sqlx::query(sql)
            .bind(address.to_string())
            .bind(start_block as i64)
            .bind(end_block as i64)
            .bind(to_str)).await?;

        for row in rows {
            let create_time: i64 = row.get("created_time");
            list.push(ERC20ContractTxResponse {
                address: row.get("address"),
                tx_hash: row.get("hash"),
                value: row.get::<i64, _>("value") as u64,
                from: row.get("_from"),
                to: row.get("_to"),
                height: row.get::<i64, _>("number") as u64,
                gas_price: row.get::<i64, _>("gas_price") as u64,
                create_time: bucky_time_to_js_time(create_time as u64),
                result: row.get::<i32, _>("result"),
                })

            }
        }

        if from_str.len() > 0
        && to_str.len() > 0 {
            let sql = r#"select address, hash, _from, _to, value, number, gas_price, created_time, result from erc20_contract_tx where address = ?1 and number between ?2 and ?3  and _from in (?4) and _to in (?5) order by number DESC"#;
            let rows = conn.query_all(sqlx::query(sql)
                .bind(address.to_string())
                .bind(start_block as i64)
                .bind(end_block as i64)
                .bind(from_str)
                .bind(to_str)).await?;

        for row in rows {
            let create_time: i64 = row.get("created_time");
            list.push(ERC20ContractTxResponse {
                address: row.get("address"),
                tx_hash: row.get("hash"),
                value: row.get::<i64, _>("value") as u64,
                from: row.get("_from"),
                to: row.get("_to"),
                height: row.get::<i64, _>("number") as u64,
                gas_price: row.get::<i64, _>("gas_price") as u64,
                create_time: bucky_time_to_js_time(create_time as u64),
                result: row.get::<i32, _>("result"),
                })

            }
        }

        Ok(list)

    }

}

#[cfg(test)]
pub mod spv_tx_storage_test {
    use std::fs::{remove_dir_all, create_dir};
    use cyfs_base::*;
    use std::convert::TryFrom;
    use cyfs_base_meta::*;
    use crate::spv_tx_storage::SPVTxStorage;
    use crate::db_helper::{map_sql_err, MetaConnectionOptions};
    use log::LevelFilter;
    use std::time::Duration;
    use sqlx::ConnectOptions;
    use crate::NFTStorage;

    pub fn create_people() -> StandardObject {
        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let public_key = private_key.public();
        StandardObject::Device(Device::new(None
                                           , UniqueId::default()
                                           , Vec::new()
                                           , Vec::new()
                                           , Vec::new()
                                           , public_key
                                           , Area::default()
                                            , DeviceCategory::OOD).build())
    }

    pub fn create_test_tx(people: &StandardObject, nonce: i64, to: &StandardObject, value: i64) -> MetaTx {
        let body = MetaTxBody::TransBalance(TransBalanceTx {
            ctid: CoinTokenId::Coin(0),
            to: vec![(to.calculate_id(), value)]
        });
        let tx = MetaTx::new(nonce, TxCaller::try_from(people).unwrap()
                         , 0
                         , 0
                         , 0
                         , None
                         , body, Vec::new()).build();
        tx
    }

    #[test]
    fn test() {
        async_std::task::block_on(async {
            let mut temp_dir = std::env::temp_dir();
            temp_dir.push("rust_test2");
            println!("{}", temp_dir.to_str().unwrap());
            if temp_dir.exists() {
                remove_dir_all(temp_dir.clone()).unwrap();
            }
            create_dir(temp_dir.clone()).unwrap();

            let mut options = MetaConnectionOptions::new().filename(temp_dir.join("spv_db")).create_if_missing(true);
            options.log_statements(LevelFilter::Off).log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
            let conn = options.connect().await.map_err(map_sql_err).unwrap();
            let storage = SPVTxStorage::new(conn);
            storage.init().await.unwrap();
            storage.init_nft_storage().await.unwrap();

            // let header = BlockDesc::new(BlockDescContent::new(ObjectId::default(), None)).build();
            let mut block_body = BlockBody::new();

            let people1 = create_people();
            let people2 = create_people();

            let tx1 = create_test_tx(&people1, 1, &people2, 10);
            block_body.add_transaction(tx1).unwrap();
            block_body.add_receipts(vec![Receipt::new(0,0)]).unwrap();

            let tx2 = create_test_tx(&people1, 1, &people2, 10);
            block_body.add_transaction(tx2).unwrap();
            block_body.add_receipts(vec![Receipt::new(0,0)]).unwrap();

            let block = Block::new(ObjectId::default(), None, HashValue::default(), block_body).unwrap().build();

            let ret = storage.add_block(&block).await;
            assert!(ret.is_ok());

            let ret = storage.get_payment_tx_list(vec![people1.calculate_id().to_string()], None, 0, 20, vec!["0".to_string()]).await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().len(), 2);

            let ret = storage.get_payment_tx_list(vec![people2.calculate_id().to_string()], None, 0, 20, vec!["0".to_string()]).await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().len(), 0);

            let ret = storage.get_collect_tx_list(vec![people1.calculate_id().to_string()], None, 0, 20, vec!["0".to_string()]).await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().len(), 0);

            let ret = storage.get_collect_tx_list(vec![people2.calculate_id().to_string()], None, 0, 20, vec!["0".to_string()]).await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().len(), 2);

            let ret = storage.get_tx_list(vec![people2.calculate_id().to_string()], None, 0, 20, vec!["0".to_string()]).await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().len(), 2);

            let ret = storage.get_file_amount(people2.calculate_id().to_string()).await;
            assert!(ret.is_ok());
            assert_eq!(ret.unwrap(), 0);
        });
    }
}
