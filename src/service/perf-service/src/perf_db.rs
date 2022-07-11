use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode};
use async_std::task;
use bson::doc;
use bson::{Document};
use mongodb::{options::ClientOptions, Client, Collection, Database};
use mongodb::error::{Error, ErrorKind, WriteFailure};
use log::*;

use async_trait::async_trait;
use crate::storage::*;
use cyfs_perf_base::*;
use std::collections::HashMap;

use crate::config::ConfigManager;

const LOCAL_MONGODB_URL: &str = "mongo_url";
const MONGO_DB_DEFAULT_NAME: &str = "db";

const MONGO_COLL_NAME_REQUEST: &str = "request";
const MONGO_COLL_NAME_ACTION: &str = "action";
const MONGO_COLL_NAME_ACC: &str = "accumulation";
const MONGO_COLL_NAME_RECORD: &str = "record";


pub const OBJECT_SELECT_MAX_PAGE_SIZE: u16 = 256;

#[derive(Clone)]
pub struct MangodbStorage{
    client: Client,
    db: Database,
    request_coll: Collection,
    action_coll: Collection,
    acc_coll: Collection,
    record_coll: Collection,
}

impl MangodbStorage {
    pub async fn new(isolate: &str) ->BuckyResult<Self> {

        let mut cfg_manager = ConfigManager::new();
        cfg_manager.init("perf_config.json").unwrap();

        let mongo_url = cfg_manager.get(LOCAL_MONGODB_URL).unwrap();

        let mut client_options = ClientOptions::parse(&mongo_url).await.unwrap();
        client_options.direct_connection = Some(true);
        client_options.app_name = Some("perf-service".to_string());

        let client = Client::with_options(client_options).map_err(|e| {
            let msg = format!("init mangodb client error: {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        let db_name = if isolate.len() > 0 {
            format!("{}-{}", isolate, MONGO_DB_DEFAULT_NAME)
        } else {
            MONGO_DB_DEFAULT_NAME.to_owned()
        };

        info!("use mango db: {}", db_name);

        let db = client.database(&db_name);
        let _ = Self::ping(&db).await;

        Self::dump_db(&db);

        // init request coll
        let request_coll = db.collection(MONGO_COLL_NAME_REQUEST);
        Self::dump_coll(&request_coll, MONGO_COLL_NAME_REQUEST.to_string());

        Self::init_coll(&db, MONGO_COLL_NAME_REQUEST.to_string()).await?;

        // init action coll
        let action_coll = db.collection(MONGO_COLL_NAME_ACTION);
        Self::dump_coll(&action_coll, MONGO_COLL_NAME_ACTION.to_string());

        Self::init_coll(&db, MONGO_COLL_NAME_ACTION.to_string()).await?;

        // init record coll
        let record_coll = db.collection(MONGO_COLL_NAME_RECORD);
        Self::dump_coll(&record_coll, MONGO_COLL_NAME_RECORD.to_string());

        Self::init_coll(&db, MONGO_COLL_NAME_RECORD.to_string()).await?;


        // init acc coll
        let acc_coll = db.collection(MONGO_COLL_NAME_ACC);
        Self::dump_coll(&acc_coll, MONGO_COLL_NAME_ACC.to_string());

        Self::init_coll(&db, MONGO_COLL_NAME_ACC.to_string()).await?;


        let ret = Self { client, db, request_coll, action_coll, acc_coll, record_coll};
        Ok(ret)
    }


    pub async fn query_index(db: &Database, coll_name: String) -> BuckyResult<()>{
        // 查询索引
        let doc = doc! {
            "listIndexes": coll_name,
        };
    
        let _ret = db.run_command(doc, None).await.map_err(|e| {
            let msg = format!("list index on coll error: {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;
    
        Ok(())
    }
    
    pub async fn init_coll(db: &Database, coll_name: String) -> BuckyResult<()> {
        const INDEX_NAME: &str = "people_id";
    
        // 建立索引
        // https://docs.mongodb.com/manual/reference/method/db.collection.createIndex/
        let doc = doc! {
            "createIndexes": coll_name,
            "indexes": [
                {
                    "key": {
                        "people_id": 1,
                    },
                    "name": INDEX_NAME,
                    "unique": false,
                },
            ],
        };
    
        let ret = db.run_command(doc, None).await.map_err(|e| {
            let msg = format!("create index on coll error: {}", e);
            error!("{}", msg);
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
    
    fn dump_coll(coll: &Collection, coll_name: String) {
        let coll = coll.clone();
        let coll_name = coll_name.clone();
        task::spawn(async move {
            match coll.count_documents(None, None).await {
                Ok(list) => {
                    info!("collection {} has doc count: {}", coll_name, list);
                }
                Err(e) => {
                    error!("count_documents error: {}", e);
                }
            }
        });
    }
    
    pub async fn ping(db: &Database) -> BuckyResult<()>{
        let doc = doc! {
            "ping": 1,
        };
    
        let _doc = db.run_command(doc, None).await.map_err(|e| {
            let _msg = format!("ping error: {}", e);
            error!("{}", e);
        }).map_err(|_e|{
            error!{"async fn ping db_run_command() failed!"};
        });

        Ok(())
    }
    

    // 判断是不是相同object_id的项目已经存在
    fn is_exists_error(e: &Error) -> bool {
        match e.kind.as_ref() {
            ErrorKind::WriteError(e) => match e {
                WriteFailure::WriteError(e) => {
                    if e.code == 11000 {
                        return true;
                    }
                }
                _ => {}
            },
            _ => {}
        }

        false
    }


    async fn insert_reqs_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        let mut doc = Document::new();
        doc.insert("device_id", device_id.to_owned());
        doc.insert("people_id", people_id.to_owned());
        doc.insert("dec_id", dec_id.to_owned());
        doc.insert("dec_name", dec_name.to_owned());
        doc.insert("version", version.to_owned());

        for (_k, v) in all {
            // device_id+dec_id+version +isolate+item_id+time
            doc.insert("isolate", v.id.clone());

            // let time_range = v.time_range.to_owned();
            // doc.insert("time_begin", time_range.begin);
            // doc.insert("time_end", time_range.end);

            let reqs = v.reqs.clone();

            let mut reqs_vec = Vec::new();
            for (_k1, v1) in reqs {
                doc.insert("item_id", v1.id);
                doc.insert("time", v1.time_range.begin.clone());
                doc.insert("time_begin", v1.time_range.begin);
                doc.insert("time_end", v1.time_range.end);
                doc.insert("total", v1.total);
                doc.insert("success", v1.success);
                doc.insert("total_time", v1.total_time);
                if v1.total_size.is_some() {
                    doc.insert("total_size", v1.total_size.unwrap());
                } else {
                    doc.insert("total_size", 0);
                }

                reqs_vec.push(doc.clone());
            }

            if !reqs_vec.is_empty() {
                let _ = self.request_coll.insert_many(reqs_vec.clone(), None).await.map_err(|e| {
                    let msg;
                    let code = if Self::is_exists_error(&e) {
                        msg = format!(
                            "insert object to req coll but already exists: {:?}, error: {}",
                            all, e
                        );
                        BuckyErrorCode::AlreadyExists
                    } else {
                        msg = format!("insert object to req coll error: {:?} {}", all, e);
                        BuckyErrorCode::MongoDBError
                    };

                    warn!("{}", msg);
                    BuckyError::new(code, msg)
                })?;

                info!("insert new to perf_request success: obj={:?}", all);

                reqs_vec.clear();
            }
        }

        Ok(())
    }

    async fn insert_action_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        let mut doc = Document::new();
        doc.insert("device_id", device_id.to_owned());
        doc.insert("people_id", people_id.to_owned());
        doc.insert("dec_id", dec_id.to_owned());
        doc.insert("dec_name", dec_name.to_owned());
        doc.insert("version", version.to_owned());

        for (_k, v) in all {
            // device_id+dec_id+version +isolate+item_id+time
            doc.insert("isolate", v.id.clone());

            // let time_range = v.time_range.clone();
            // doc.insert("time_begin", time_range.begin);
            // doc.insert("time_end", time_range.end);

            let actions = v.actions.clone();

            let mut actions_vec = Vec::new();
            for action in actions {
                doc.insert("item_id", action.id);
                doc.insert("time", action.time);
                doc.insert("err", action.err);
                doc.insert("name", action.name);
                doc.insert("value", action.value);

                actions_vec.push(doc.clone());
            }

            if !actions_vec.is_empty() {
                let _ = self.action_coll.insert_many(actions_vec.clone(), None).await.map_err(|e| {
                    let msg;
                    let code = if Self::is_exists_error(&e) {
                        msg = format!(
                            "insert object to action coll but already exists: {:?}, error: {}",
                            all, e
                        );
                        BuckyErrorCode::AlreadyExists
                    } else {
                        msg = format!("insert object to action coll error: {:?} {}", all, e);
                        BuckyErrorCode::MongoDBError
                    };

                    warn!("{}", msg);
                    BuckyError::new(code, msg)
                })?;

                info!("insert new to perf_action success: obj={:?}", all);

                actions_vec.clear();
            }

        }

        Ok(())
    }


    async fn insert_acc_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        let mut doc = Document::new();
        doc.insert("device_id", device_id.to_owned());
        doc.insert("people_id", people_id.to_owned());
        doc.insert("dec_id", dec_id.to_owned());
        doc.insert("dec_name", dec_name.to_owned());
        doc.insert("version", version.to_owned());

        for (_k, v) in all {
            // device_id+dec_id+version +isolate+item_id+time
            doc.insert("isolate", v.id.clone());

            // let time_range = v.time_range.to_owned();
            // doc.insert("time_begin", time_range.begin);
            // doc.insert("time_end", time_range.end);

            let accs = v.accumulations.clone();

            let mut acc_vec = Vec::new();
            for (_k1, v1) in accs {
                doc.insert("item_id", v1.id);
                doc.insert("time", v1.time_range.begin.clone());
                doc.insert("time_begin", v1.time_range.begin);
                doc.insert("time_end", v1.time_range.end);
                doc.insert("total", v1.total);
                doc.insert("success", v1.success);
                if v1.total_size.is_some() {
                    doc.insert("total_size", v1.total_size.unwrap());
                } else {
                    doc.insert("total_size", 0);
                }

                acc_vec.push(doc.clone());
            }

            if !acc_vec.is_empty() {
                let _ = self.acc_coll.insert_many(acc_vec.clone(), None).await.map_err(|e| {
                    let msg;
                    let code = if Self::is_exists_error(&e) {
                        msg = format!(
                            "insert object to acc coll but already exists: {:?}, error: {}",
                            all, e
                        );
                        BuckyErrorCode::AlreadyExists
                    } else {
                        msg = format!("insert object to acc coll error: {:?} {}", all, e);
                        BuckyErrorCode::MongoDBError
                    };

                    warn!("{}", msg);
                    BuckyError::new(code, msg)
                })?;

                info!("insert new to perf_accumulation success: obj={:?}", all);
                acc_vec.clear();
            }
        }

        Ok(())
    }


    async fn insert_record_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        let mut doc = Document::new();
        doc.insert("device_id", device_id.to_owned());
        doc.insert("people_id", people_id.to_owned());
        doc.insert("dec_id", dec_id.to_owned());
        doc.insert("dec_name", dec_name.to_owned());
        doc.insert("version", version.to_owned());

        for (_k, v) in all {
            // device_id+dec_id+version +isolate+item_id+time
            doc.insert("isolate", v.id.clone());

            // let time_range = v.time_range.to_owned();
            // doc.insert("time_begin", time_range.begin);
            // doc.insert("time_end", time_range.end);

            let records = v.records.clone();

            let mut record_vec = Vec::new();
            for (_k1, v1) in records {
                doc.insert("item_id", v1.id);
                doc.insert("time", v1.time);
                doc.insert("total", v1.total);
                if v1.total_size.is_some() {
                    doc.insert("total_size", v1.total_size.unwrap());
                }else {
                    doc.insert("total_size", 0);
                }

                record_vec.push(doc.clone());
            }

            if !record_vec.is_empty() {
                let _ = self.record_coll.insert_many(record_vec.clone(), None).await.map_err(|e| {
                    let msg;
                    let code = if Self::is_exists_error(&e) {
                        msg = format!(
                            "insert object to record coll but already exists: {:?}, error: {}",
                            all, e
                        );
                        BuckyErrorCode::AlreadyExists
                    } else {
                        msg = format!("insert object to record coll error: {:?} {}", all, e);
                        BuckyErrorCode::MongoDBError
                    };

                    warn!("{}", msg);
                    BuckyError::new(code, msg)
                })?;

                info!("insert new to perf_record success: obj={:?}", all);

                record_vec.clear();
            }
        }

        Ok(())
    }


    pub async fn insert_entity_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {

        let _ = self.insert_reqs_list(people_id.clone(), device_id.clone(), dec_id.clone(), dec_name.clone(), version.clone(), all).await;
        let _ = self.insert_action_list(people_id.clone(), device_id.clone(), dec_id.clone(), dec_name.clone(), version.clone(), all).await;
        let _ = self.insert_acc_list(people_id.clone(), device_id.clone(), dec_id.clone(), dec_name.clone(), version.clone(), all).await;
        let _ = self.insert_record_list(people_id.clone(), device_id.clone(), dec_id.clone(), dec_name.clone(), version.clone(), all).await;

        Ok(())
    }

}

#[async_trait]
impl Storage for MangodbStorage {
    async fn insert_entity_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        self.insert_entity_list(people_id, device_id, dec_id, dec_name, version,  all).await
    }

    fn clone(&self) -> Box<dyn Storage> {
        Box::new(Clone::clone(&self as &MangodbStorage)) as Box<dyn Storage>
    }
}