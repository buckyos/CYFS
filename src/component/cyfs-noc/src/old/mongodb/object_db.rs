use cyfs_base::{BuckyError, BuckyResult};
use cyfs_lib::NamedObjectCacheStat;

use async_std::task;
use bson::doc;
use mongodb::{options::ClientOptions, Client, Collection, Database};

const LOCAL_MONGODB_URL: &str = "mongodb://localhost:27017";
const MONGO_DB_DEFAULT_NAME: &str = "named-objects";
const MONGO_COLL_NAME: &str = "default";

#[derive(Clone)]
pub(crate) struct ObjectDB {
    client: Client,
    db: Database,
    pub coll: Collection,
}

impl ObjectDB {
    pub async fn new(isolate: &str) -> BuckyResult<Self> {
        let mut client_options = ClientOptions::parse(LOCAL_MONGODB_URL).await.unwrap();
        client_options.direct_connection = Some(true);
        client_options.app_name = Some("named-data-cache".to_string());

        // mongodb初始化时候会导致调用栈过深，避免溢出
        /*let client = async_std::task::spawn(async move {
            Client::with_options(client_options).map_err(|e| {
                let msg = format!("init mongodb client error: {}", e);
                error!("{}", msg);
                BuckyError::from(msg)
            })
        }).await?;
        */

        let client = Client::with_options(client_options).map_err(|e| {
            let msg = format!("init mongodb client error: {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;
        
        // 如果指定了isolate，那么需要和默认db隔离
        let db_name = if isolate.len() > 0 {
            format!("{}-{}", isolate, MONGO_DB_DEFAULT_NAME)
        } else {
            MONGO_DB_DEFAULT_NAME.to_owned()
        };

        info!("will use mongo db: {}", db_name);
        let db = client.database(&db_name);
        Self::dump_db(&db);

        let coll = db.collection(MONGO_COLL_NAME);
        Self::dump_coll(&coll);

        Self::init_coll(&db).await?;

        let ret = Self { client, db, coll };
        Ok(ret)
    }

    pub async fn query_index(db: &Database) -> BuckyResult<()> {
        // 查询索引
        let doc = doc! {
            "listIndexes": MONGO_COLL_NAME,
        };

        let _ret = db.run_command(doc, None).await.map_err(|e| {
            let msg = format!("list index on coll error: {}", e);
            error!("{}", e);

            BuckyError::from(msg)
        })?;

        // TODO

        Ok(())
    }

    pub async fn init_coll(db: &Database) -> BuckyResult<()> {
        const INDEX_NAME: &str = "object_id";

        // 建立索引
        // https://docs.mongodb.com/manual/reference/method/db.collection.createIndex/
        let doc = doc! {
            "createIndexes": MONGO_COLL_NAME,
            "indexes": [
                {
                    "key": {
                        "object_id": 1,
                    },
                    "name": INDEX_NAME,
                    "unique": true,
                },
            ],
        };

        let ret = db.run_command(doc, None).await.map_err(|e| {
            let msg = format!("create index on coll error: {}", e);
            error!("{}", e);

            BuckyError::from(msg)
        })?;

        trace!("{}", ret);

        let ok = ret.get_f64("ok").unwrap();
        if ok == 1.0 {
            if let Ok(note) = ret.get_str("note") {
                info!("index already exists: {} {}", INDEX_NAME, note);
            } else {
                info!("create index success: {}", INDEX_NAME);
            }
        } else {
            if let Ok(note) = ret.get_str("note") {
                info!("index already exists: {} {}", INDEX_NAME, note);
            } else {
                let err = ret.get("errmsg");
                let code = ret.get("code");

                let msg = format!("create index error: {} {:?} {:?}", INDEX_NAME, err, code);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        Ok(())
    }

    fn dump_db(db: &Database) {
        let db = db.clone();
        task::spawn(async move {
            match db.list_collection_names(None).await {
                Ok(list) => {
                    info!("collections in {} as follows: {:?}", db.name(), list);
                }
                Err(e) => {
                    error!("list_collection_names error: {}", e);
                }
            }
        });
    }

    fn dump_coll(coll: &Collection) {
        let coll = coll.clone();
        task::spawn(async move {
            match coll.count_documents(None, None).await {
                Ok(list) => {
                    info!("collection {} has doc count: {}", MONGO_COLL_NAME, list);
                }
                Err(e) => {
                    error!("count_documents error: {}", e);
                }
            }
        });
    }

    pub async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        let doc = doc! {
            "collStats": MONGO_COLL_NAME,
            "scale": 1024,
        };

        let doc = self.db.run_command(doc, None).await.map_err(|e| {
            let msg = format!("stat coll error: {}", e);
            error!("{}", e);

            BuckyError::from(msg)
        })?;


        // info!("doc {:?}", doc.get("storageSize"));
       
        let count = doc.get_i32("count").unwrap();
        let storage_size = doc.get_i32("storageSize").unwrap();

        let stat = NamedObjectCacheStat {
            count: count as u64,
            storage_size: storage_size as u64 * 1024,
        };

        Ok(stat)
        /*
        match self.db.coll.estimated_document_count(None).await {
            Ok(count) => {
                debug!("count object count: {}", count);
                Ok(count)
            }
            Err(e) => {
                let msg = format!(
                    "count objects error: {}", e
                );
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
        */
    }
}
