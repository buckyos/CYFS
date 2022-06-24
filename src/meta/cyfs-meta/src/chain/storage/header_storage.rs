use std::path::{PathBuf, Path};
use cyfs_base::*;
use cyfs_base_meta::*;
use log::*;
use crate::*;
use sqlx::{ConnectOptions, Row};
use std::time::Duration;
use sqlx::sqlite::SqliteJournalMode;

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

pub struct BlockHeaderStorage {
    db_path: PathBuf
}

static INSERT_HEADER_SQL: &str = "INSERT INTO headers (hash, pre, raw, verified) VALUES(?1, ?2, ?3, ?4)";
static EXTEND_BEST_SQL: &str = "INSERT INTO best (hash, number, timestamp) VALUES(?1, ?2, ?3)";

impl BlockHeaderStorage {
    pub async fn new(path: PathBuf) -> BuckyResult<Self> {
        let storage = BlockHeaderStorage {
            db_path: path
        };
        storage.init().await?;
        Ok(storage)
    }

    async fn get_conn(&self, _read_only: bool) -> BuckyResult<MetaConnection> {
        let mut options = MetaConnectionOptions::new().filename(self.db_path.as_path()).create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory).busy_timeout(Duration::new(10, 0));
        options.log_statements(LevelFilter::Off).log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
        options.connect().await.map_err(map_sql_err)
    }

    // fn release_conn(&self, conn: rusqlite::Connection) {
    //     conn.close();
    // }

    async fn init(&self) -> BuckyResult<()> {
        static INIT_HEADER_TBL_SQL: &str = "CREATE TABLE IF NOT EXISTS \"headers\"(
            \"hash\" CHAR(64) PRIMARY KEY NOT NULL UNIQUE,
            \"pre\" CHAR(64) NOT NULL,
            \"verified\" TINYINT NOT NULL,
            \"raw\" BLOB NOT NULL);";

        static INIT_BEST_TBL_SQL: &str = "CREATE TABLE IF NOT EXISTS \"best\"(
            \"number\" INTEGER PRIMARY KEY NOT NULL UNIQUE,
            \"hash\" CHAR(64) NOT NULL,
            \"timestamp\" INTEGER NOT NULL);";
        let mut conn = self.get_conn(false).await?;
        conn.execute_sql(sqlx::query(INIT_HEADER_TBL_SQL)).await?;
        conn.execute_sql(sqlx::query(INIT_BEST_TBL_SQL)).await?;
        info!("header storage init success");

        Ok(())
    }

    pub async fn load_header_by_number(&self, n: i64) -> BuckyResult<BlockDesc> {
        static QUERY_HEADER_BY_NUMBER_SQL: &str = "SELECT raw FROM headers WHERE hash IN (SELECT hash FROM best where number=?1)";

        let mut conn = self.get_conn(true).await?;
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

        let mut conn = self.get_conn(true).await?;
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

        let mut conn = self.get_conn(true).await?;
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

        let mut conn = self.get_conn(false).await?;
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

        let mut conn = self.get_conn(false).await?;
        conn.execute_sql(sqlx::query(INSERT_HEADER_SQL)
            .bind(header.hash_str())
            .bind(header.pre_block_hash_str())
            .bind(raw_header.as_slice())
            .bind(BlockVerifyState::NotVerified as i16)).await?;
        Ok(())
    }

    pub async fn save_genesis(&self, header: &BlockDesc) -> BuckyResult<()> {
        let r = header.to_vec();
        if r.is_err() {
            let err = r.err().unwrap();
            error!("serialize header error: {:?}", &err);
            return Err(err);
        }
        let raw_header = r.ok().unwrap();
        let mut conn = self.get_conn(false).await?;
        conn.execute_sql(sqlx::query(INSERT_HEADER_SQL)
            .bind(header.hash_str())
            .bind(header.pre_block_hash_str())
            .bind(raw_header)
            .bind(BlockVerifyState::Verified as i16)).await?;
        conn.execute_sql(sqlx::query(EXTEND_BEST_SQL)
            .bind(header.hash_str())
            .bind(header.number())
            .bind(header.create_time() as i64)).await?;

        Ok(())
    }

    fn path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub fn backup(&self, height: i64) -> BuckyResult<()> {
        if height > 5 {
            let backup_file = PathBuf::from(format!("{}_{}", self.path().to_str().unwrap(), height - 5));
            if backup_file.exists() {
                let _ = std::fs::remove_file(backup_file);
            }
        }
        let backup_file = format!("{}_{}", self.path().to_str().unwrap(), height);

        std::fs::copy(self.path(), backup_file).map_err(|err| meta_err!({
            error!("backup file {} fail.height {}, err {}", self.path().display(), height, err);
            ERROR_NOT_FOUND
        }))?;
        Ok(())
    }

    pub fn recovery(&self, height: i64) -> BuckyResult<()> {
        let backup_file = format!("{}_{}", self.path().to_str().unwrap(), height);

        std::fs::copy(backup_file, self.path()).map_err(|err| meta_err!({
            error!("recovery file {} fail.height {}, err {}", self.path().display(), height, err);
            ERROR_NOT_FOUND
        }))?;
        Ok(())
    }

    pub fn backup_exist(&self, height: i64) -> bool {
        let backup_file = format!("{}_{}", self.path().to_str().unwrap(), height);
        Path::new(backup_file.as_str()).exists()
    }
}
