use super::object_db::ObjectDB;
use super::super::common::*;
use super::super::named_object_storage::*;
use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, DeviceId, ObjectId, RawDecode, RawEncode,
};
use cyfs_lib::*;

use async_std::prelude::*;
use async_trait::async_trait;
use bson::{bson, doc};
use bson::{document::ValueAccessError, Document};
use mongodb::error::{Error, ErrorKind, WriteFailure};
use mongodb::options::FindOptions;
use std::str::FromStr;
use std::sync::Arc;

pub struct ObjectDBCache {
    db: ObjectDB,
    updater: Arc<NOCUpdater>,
}

impl Clone for ObjectDBCache {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            updater: self.updater.clone(),
        }
    }
}

impl ObjectDBCache {
    pub async fn new(
        isolate: &str,
        insert_object_event: InsertObjectEventManager,
    ) -> BuckyResult<Self> {
        let updater = NOCUpdater::new(insert_object_event);

        match ObjectDB::new(isolate).await {
            Ok(db) => Ok(Self {
                db,
                updater: Arc::new(updater),
            }),
            Err(e) => {
                error!("init object mongo db error: {}", e);

                Err(e)
            }
        }
    }

    pub async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.db.stat().await
    }

    pub async fn insert(
        &self,
        req: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        assert!(req.object_raw.is_some());
        assert!(req.insert_time > 0);

        self.update(req, event).await
    }

    async fn replace_old(
        &self,
        req: &ObjectCacheData,
        old: &ObjectCacheData,
    ) -> BuckyResult<usize> {
        assert!(req.object_id == old.object_id);
        let mut query = Document::new();
        query.insert("object_id", Self::to_bin(&req.object_id)?);
        query.insert("update_time", old.update_time);
        let doc = Self::object_data_to_doc(req)?;

        match self.db.coll.replace_one(query, doc, None).await {
            Ok(ret) => {
                // 如果查找不到，说明发生了竞争，需要重试
                if ret.matched_count != 1 {
                    warn!(
                        "replace but not found, now will retry! obj={} cur_update_time={}",
                        req.object_id, old.update_time
                    );

                    return Ok(0);
                }

                if ret.modified_count != 1 {
                    let msg = format!(
                        "replace found but modify failed! obj={}, update_time={}",
                        req.object_id, old.update_time
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg));
                }

                debug!(
                    "replace obj success! obj={} cur={} new={}",
                    req.object_id, old.update_time, req.update_time
                );

                Ok(1)
            }
            Err(e) => {
                // FIXME 出错后需要重试与否？
                let msg = format!("replace object failed! obj={}, err={}", req.object_id, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    async fn insert_new(&self, req: &ObjectCacheData) -> BuckyResult<usize> {
        let doc = Self::object_data_to_doc(req)?;

        let _ = self.db.coll.insert_one(doc, None).await.map_err(|e| {
            let msg;
            let code = if Self::is_exists_error(&e) {
                msg = format!(
                    "insert object to coll but already exists: {}",
                    req.object_id
                );
                BuckyErrorCode::AlreadyExists
            } else {
                msg = format!("insert object to coll error: {} {}", req.object_id, e);
                BuckyErrorCode::MongoDBError
            };

            warn!("{}", msg);
            BuckyError::new(code, msg)
        })?;

        info!("insert new to noc success: obj={}", req.object_id);

        Ok(1)
    }

    // 更新签名，同时更新insert_time为当前时间
    async fn update_signs(&self, req: &ObjectCacheData, insert_time: &u64) -> BuckyResult<usize> {
        assert!(req.object_raw.is_some());
        assert!(req.insert_time > 0);
        assert!(*insert_time > req.insert_time);

        let mut filter = Document::new();
        filter.insert("object_id", Self::to_bin(&req.object_id)?);
        filter.insert("insert_time", req.insert_time);

        // 只需要更新object_raw和insert_time两个字段
        let mut doc = Document::new();
        // object_buf以二进制形式存储
        let bin = bson::Binary {
            bytes: req.object_raw.as_ref().unwrap().clone(),
            subtype: bson::spec::BinarySubtype::Generic,
        };

        doc.insert("object", bin);
        doc.insert("insert_time", insert_time);
        let doc = doc! {"$set" : doc};

        match self.db.coll.find_one_and_update(filter, doc, None).await {
            Ok(Some(_)) => Ok(1),
            Ok(None) => Ok(0),
            Err(e) => {
                let msg = format!(
                    "update object error: id={}, insert_time={}, {}",
                    req.object_id, req.insert_time, e
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    async fn update(
        &self,
        req: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        self.updater.update(self, req, event).await
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

    async fn try_get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        let mut filter = Document::new();

        filter.insert("object_id", Self::to_bin(object_id)?);

        match self.db.coll.find_one(Some(filter), None).await {
            Ok(Some(doc)) => {
                let req = Self::object_data_from_doc(doc)?;
                Ok(Some(req))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                let msg = format!("get object error: {} {}", object_id, e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    pub async fn get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        match self.try_get(object_id).await {
            Ok(Some(obj)) => Ok(Some(obj)),
            Ok(None) => {
                info!("object not found: {}", object_id);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    async fn try_delete(&self, object_id: &ObjectId) -> BuckyResult<u32> {
        let mut query = Document::new();

        query.insert("object_id", Self::to_bin(object_id)?);

        match self.db.coll.delete_one(query, None).await {
            Ok(ret) => {
                if ret.deleted_count == 1 {
                    info!("delete object success: obj={}", object_id);
                    Ok(1)
                } else {
                    info!("delete object but not found! obj={}", object_id);

                    Ok(0)
                }
            }
            Err(e) => {
                let msg = format!("delete object error! obj={}, err={}", object_id, e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    async fn try_find_and_delete(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        let mut filter = Document::new();

        filter.insert("object_id", Self::to_bin(object_id)?);

        match self.db.coll.find_one_and_delete(filter, None).await {
            Ok(Some(doc)) => {
                info!("find and delete object success: obj={}", object_id);
                let req = Self::object_data_from_doc(doc)?;
                Ok(Some(req))
            }
            Ok(None) => {
                info!("find and delete object but not found: obj={}", object_id);
                Ok(None)
            }
            Err(e) => {
                let msg = format!("delete object error: {} {}", object_id, e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    pub async fn delete(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        if req.flags & CYFS_REQUEST_FLAG_DELETE_WITH_QUERY != 0 {
            let ret = self.try_find_and_delete(&req.object_id).await?;
            if ret.is_some() {
                Ok(NamedObjectCacheDeleteObjectResult {
                    deleted_count: 1,
                    object: ret,
                })
            } else {
                Ok(NamedObjectCacheDeleteObjectResult {
                    deleted_count: 0,
                    object: None,
                })
            }
        } else {
            let deleted_count = self.try_delete(&req.object_id).await?;
            let ret = NamedObjectCacheDeleteObjectResult {
                deleted_count,
                object: None,
            };
            Ok(ret)
        }
    }

    fn timerange_to_doc(
        name: &str,
        range: &NamedObjectCacheSelectObjectTimeRange,
        doc: &mut Document,
    ) {
        let value = if range.begin.is_some() && range.end.is_some() {
            bson!({ "$gte": *range.begin.as_ref().unwrap(), "$lt": *range.end.as_ref().unwrap() })
        } else if range.begin.is_some() {
            bson!({ "$gte": *range.begin.as_ref().unwrap() })
        } else if range.end.is_some() {
            bson!({ "$lt": *range.end.as_ref().unwrap() })
        } else {
            return;
        };

        doc.insert(name.to_owned(), value);
    }

    fn select_filter_to_doc(filter: &NamedObjectCacheSelectObjectFilter) -> BuckyResult<Document> {
        let mut doc = Document::new();

        if filter.obj_type.is_some() {
            doc.insert("obj_type", *filter.obj_type.as_ref().unwrap() as i32);
        }

        if filter.obj_type_code.is_some() {
            doc.insert(
                "obj_type_code",
                filter.obj_type_code.as_ref().unwrap().to_u16() as i32,
            );
        }
        if filter.dec_id.is_some() {
            doc.insert("dec_id", Self::to_bin(filter.dec_id.as_ref().unwrap())?);
        }
        if filter.owner_id.is_some() {
            doc.insert("owner_id", Self::to_bin(filter.owner_id.as_ref().unwrap())?);
        }
        if filter.author_id.is_some() {
            doc.insert(
                "author_id",
                Self::to_bin(filter.author_id.as_ref().unwrap())?,
            );
        }

        if filter.create_time.is_some() {
            Self::timerange_to_doc(
                "create_time",
                filter.create_time.as_ref().unwrap(),
                &mut doc,
            );
        }
        if filter.update_time.is_some() {
            Self::timerange_to_doc(
                "update_time",
                filter.update_time.as_ref().unwrap(),
                &mut doc,
            );
        }
        if filter.insert_time.is_some() {
            Self::timerange_to_doc(
                "insert_time",
                filter.insert_time.as_ref().unwrap(),
                &mut doc,
            );
        }
        Ok(doc)
    }

    fn select_opt_to_find_opt(
        opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<FindOptions> {
        let opt = if opt.is_some() {
            opt.unwrap().to_owned()
        } else {
            NamedObjectCacheSelectObjectOption::default()
        };

        if opt.page_size > OBJECT_SELECT_MAX_PAGE_SIZE {
            let msg = format!(
                "invalid page_size: {}, max={}",
                opt.page_size, OBJECT_SELECT_MAX_PAGE_SIZE
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let mut find_opt = FindOptions::default();
        find_opt.skip = Some(opt.page_size as i64 * opt.page_index as i64);
        find_opt.limit = Some(opt.page_size as i64);

        // select按照insert_time由高到低排序
        find_opt.sort = Some(doc! { "insert_time": -1 });

        Ok(find_opt)
    }

    pub async fn select(
        &self,
        filter: &NamedObjectCacheSelectObjectFilter,
        opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        let filter = Self::select_filter_to_doc(filter)?;

        let opt = Self::select_opt_to_find_opt(opt)?;

        match self.db.coll.find(filter, opt).await {
            Ok(mut cursor) => {
                let mut list = Vec::new();
                while let Some(doc) = cursor.next().await {
                    match doc {
                        Ok(doc) => match Self::object_data_from_doc(doc) {
                            Ok(data) => {
                                list.push(data);
                            }
                            Err(e) => {
                                error!("decode object data from doc error: err={}", e);
                            }
                        },
                        Err(e) => {
                            // 每次next都可能会触发网络操作，所以会导致失败，出错后如何处理？我们先中断
                            error!("fetch next select doc error: {}", e);
                            break;
                        }
                    }
                }

                Ok(list)
            }

            Err(e) => {
                let msg = format!("select object error: filter=, {}", e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }

        //unreachable!();
    }

    fn from_bin<'de, T>(bin: &'de Vec<u8>) -> BuckyResult<T>
    where
        T: RawDecode<'de>,
    {
        let (obj, _) = T::raw_decode(&bin[..])?;

        Ok(obj)
    }

    fn get_item_from_doc<'de, T>(doc: &'de Document, id: &str) -> BuckyResult<T>
    where
        T: RawDecode<'de>,
    {
        match doc.get_binary_generic(id) {
            Ok(buf) => Self::from_bin(buf),
            Err(ValueAccessError::NotPresent) => {
                let msg = format!("get binary from doc not found: {}", id);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => {
                let msg = format!("get binary from doc error: {} {}", id, e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn get_opt_item_from_doc<'de, T>(doc: &'de Document, id: &str) -> BuckyResult<Option<T>>
    where
        T: RawDecode<'de>,
    {
        match doc.get_binary_generic(id) {
            Ok(buf) => match Self::from_bin(buf) {
                Ok(obj) => Ok(Some(obj)),
                Err(e) => {
                    error!("decode object from doc bin error: {} {}", id, e);
                    Err(e)
                }
            },
            Err(ValueAccessError::NotPresent) => Ok(None),
            Err(e) => {
                let msg = format!("get binary from doc error: {} {}", id, e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn to_bin(obj: &impl RawEncode) -> BuckyResult<bson::Binary> {
        let size = obj.raw_measure(&None)?;
        let mut buf: Vec<u8> = Vec::with_capacity(size);
        unsafe {
            buf.set_len(size);
        }

        obj.raw_encode(&mut buf[..], &None)?;

        let bin = bson::Binary {
            bytes: buf,
            subtype: bson::spec::BinarySubtype::Generic,
        };

        Ok(bin)
    }

    fn object_data_from_doc(doc: Document) -> BuckyResult<ObjectCacheData> {
        let object_id: ObjectId = Self::get_item_from_doc(&doc, "object_id")?;

        let protocol = doc.get_str("protocol").map_err(|e| {
            let msg = format!("get binary from doc not found: protcol {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        // TODO 移除protocol字段
        let protocol = match RequestProtocol::from_str(protocol) {
            Ok(v) => v,
            Err(_e) => RequestProtocol::Native,
        };

        let source: DeviceId = Self::get_item_from_doc(&doc, "device_id")?;

        let flags = doc.get_i32("flags").unwrap() as u32;

        // 这里只读取insert_time，create_time和update_time直接从对象内部获取
        let insert_time: u64 = doc.get_i64("insert_time").unwrap() as u64;

        let object_raw = doc.get_binary_generic("object").map_err(|e| {
            let msg = format!("get binary from doc not found: object {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        let dec_id: Option<ObjectId> = Self::get_opt_item_from_doc(&doc, "dec_id")?;

        // FIXME 为了兼容之前的没有rank的数据库，如果不存在的话先使用OBJECT_RANK_NONE
        let rank = doc.get_i32("rank").unwrap_or(100) as u8;
        let mut data = ObjectCacheData {
            protocol,
            source,
            object_id,
            dec_id,
            object_raw: Some(object_raw.clone()),
            object: None,
            flags,
            create_time: 0,
            update_time: 0,
            insert_time,
            rank,
        };

        data.rebuild_object()?;

        Ok(data)
    }

    fn object_data_to_doc(req: &ObjectCacheData) -> BuckyResult<Document> {
        assert!(req.object_raw.is_some());
        assert!(req.object.is_some());

        let mut doc = Document::new();

        doc.insert("object_id", Self::to_bin(&req.object_id)?);

        // 创建时间/修改时间/插入时间(bson::i64)
        doc.insert("create_time", req.create_time);

        doc.insert("update_time", req.update_time);

        assert!(req.insert_time > 0);
        doc.insert("insert_time", req.insert_time);

        let procotol = req.protocol.to_string();
        doc.insert("protocol", procotol);

        doc.insert("device_id", Self::to_bin(&req.source)?);

        doc.insert("flags", req.flags);

        // object_buf以二进制形式存储
        let bin = bson::Binary {
            bytes: req.object_raw.as_ref().unwrap().clone(),
            subtype: bson::spec::BinarySubtype::Generic,
        };

        doc.insert("object", bin);

        // 添加object的一些可选字段
        let object = req.object.as_ref().unwrap();

        doc.insert("obj_type", object.obj_type() as i32);
        doc.insert("obj_type_code", object.obj_type_code().to_u16() as i32);

        if req.dec_id.is_some() {
            doc.insert("dec_id", Self::to_bin(req.dec_id.as_ref().unwrap())?);
        } else if let Some(id) = object.dec_id() {
            doc.insert("dec_id", Self::to_bin(id)?);
        }

        if let Some(id) = object.owner() {
            doc.insert("owner_id", Self::to_bin(id)?);
        }
        if let Some(id) = object.author() {
            doc.insert("author_id", Self::to_bin(id)?);
        }

        doc.insert("rank", req.rank as i32);

        debug!("new mongo doc: {}", req);

        Ok(doc)
    }

    pub async fn list(
        &self,
        begin_seq: u64,
        end_seq: u64,
        page_index: u16,
        page_size: u16,
    ) -> BuckyResult<Vec<SyncObjectData>> {
        let filter = Self::list_filter_to_doc(begin_seq, end_seq)?;

        let opt = Self::list_opt_to_find_opt(page_index, page_size)?;

        match self.db.coll.find(filter, opt).await {
            Ok(mut cursor) => {
                let mut list = Vec::new();
                while let Some(doc) = cursor.next().await {
                    match doc {
                        Ok(doc) => match Self::sync_object_data_from_doc(doc) {
                            Ok(data) => {
                                list.push(data);
                            }
                            Err(e) => {
                                error!("decode list object data from doc error: err={}", e);
                            }
                        },
                        Err(e) => {
                            // 每次next都可能会触发网络操作，所以会导致失败，出错后如何处理？我们先中断
                            error!("fetch next list object doc error: {}", e);
                            break;
                        }
                    }
                }

                Ok(list)
            }

            Err(e) => {
                let msg = format!("list object error: filter=, {}", e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    fn list_filter_to_doc(begin_seq: u64, end_seq: u64) -> BuckyResult<Document> {
        let mut doc = Document::new();

        // 按seq区间过滤
        let value = bson!({ "$gte": begin_seq, "$lte": end_seq });
        doc.insert("insert_time".to_owned(), value);

        Ok(doc)
    }

    fn list_opt_to_find_opt(page_index: u16, page_size: u16) -> BuckyResult<FindOptions> {
        if page_size > OBJECT_SELECT_MAX_PAGE_SIZE {
            let msg = format!(
                "invalid page_size: {}, max={}",
                page_size, OBJECT_SELECT_MAX_PAGE_SIZE
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let mut find_opt = FindOptions::default();
        find_opt.skip = Some(page_size as i64 * page_index as i64);
        find_opt.limit = Some(page_size as i64);

        // 按照seq(insert_time)由低到高排序
        find_opt.sort = Some(doc! { "insert_time": 1 });

        Ok(find_opt)
    }

    fn sync_object_data_from_doc(doc: Document) -> BuckyResult<SyncObjectData> {
        let object_id: ObjectId = Self::get_item_from_doc(&doc, "object_id")?;

        let insert_time: u64 = doc.get_i64("insert_time").unwrap() as u64;
        let update_time: u64 = doc.get_i64("update_time").unwrap() as u64;

        let data = SyncObjectData {
            object_id,
            update_time,
            seq: insert_time,
        };

        Ok(data)
    }

    pub async fn get_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        list: &Vec<ObjectId>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        let filter = Self::get_objects_filter_to_doc(begin_seq, end_seq, list)?;

        let opt = Self::get_objects_opt_to_find_opt()?;

        match self.db.coll.find(filter, opt).await {
            Ok(mut cursor) => {
                let mut list = Vec::new();
                while let Some(doc) = cursor.next().await {
                    match doc {
                        Ok(doc) => match Self::object_data_from_doc(doc) {
                            Ok(data) => {
                                list.push(data);
                            }
                            Err(e) => {
                                error!("decode object data from doc error: err={}", e);
                            }
                        },
                        Err(e) => {
                            // 每次next都可能会触发网络操作，所以会导致失败，出错后如何处理？我们先中断
                            error!("fetch next get objects doc error: {}", e);
                            break;
                        }
                    }
                }

                Ok(list)
            }

            Err(e) => {
                let msg = format!("get objects error: filter=, {}", e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    fn get_objects_filter_to_doc(
        begin_seq: u64,
        end_seq: u64,
        list: &Vec<ObjectId>,
    ) -> BuckyResult<Document> {
        let mut doc = Document::new();

        // 限定seq区间
        let value = bson!({ "$gte": begin_seq, "$lte": end_seq });
        doc.insert("insert_time".to_owned(), value);

        // 不再通过rank过滤
        // let value = bson!({ "$gte": OBJECT_RANK_SYNC_LEVEL as i32 });
        // doc.insert("rank".to_owned(), value);

        // 指定查询的object_id列表
        let mut id_list = Vec::new();
        for id in list {
            id_list.push(Self::to_bin(id)?);
        }

        let value = bson!({ "$in": id_list });

        doc.insert("object_id".to_owned(), value);

        Ok(doc)
    }

    fn get_objects_opt_to_find_opt() -> BuckyResult<FindOptions> {
        let mut find_opt = FindOptions::default();
        // 按照seq(insert_time)由低到高排序
        find_opt.sort = Some(doc! { "insert_time": 1 });

        Ok(find_opt)
    }

    async fn get_latest_seq(&self) -> BuckyResult<u64> {
        // 只需要获取符合rank的第一个对象即可

        let mut filter = Document::new();
        let value = bson!({ "$gte": 60 as i32 });
        filter.insert("rank".to_owned(), value);

        let mut find_opt = FindOptions::default();
        find_opt.skip = Some(0);
        find_opt.limit = Some(1);

        // 按照seq(insert_time)由高到低排序
        find_opt.sort = Some(doc! { "insert_time": -1 });

        match self.db.coll.find(filter, find_opt).await {
            Ok(mut cursor) => {
                if let Some(doc) = cursor.next().await {
                    match doc {
                        Ok(doc) => {
                            let insert_time: u64 = doc.get_i64("insert_time").unwrap() as u64;
                            info!("get_latest_seq={}", insert_time);

                            Ok(insert_time)
                        }
                        Err(e) => {
                            // 每次next都可能会触发网络操作，所以会导致失败，出错后如何处理？我们先中断
                            let msg = format!("get_latest_seq error: {}", e);
                            error!("{}", msg);
                            Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
                        }
                    }
                } else {
                    info!("noc is empty, get_latest_seq=0");

                    Ok(0)
                }
            }

            Err(e) => {
                let msg = format!("get_latest_seq error: {}", e);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    async fn query_object(&self, object_id: &ObjectId, update_time: &u64) -> BuckyResult<bool> {
        let mut filter = Document::new();

        filter.insert("object_id", Self::to_bin(object_id)?);
        filter.insert("update_time", update_time);

        match self.db.coll.find_one(Some(filter), None).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => {
                let msg = format!(
                    "get object error: id={}, update_time={}, {}",
                    object_id, update_time, e
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg))
            }
        }
    }

    // 判断一组对象是否存在，存在的话更新zone_seq
    async fn diff_objects(&self, list: &Vec<SyncObjectData>) -> BuckyResult<Vec<bool>> {
        let mut result = Vec::with_capacity(list.len());

        for item in list {
            let mut filter = Document::new();
            filter.insert("object_id", Self::to_bin(&item.object_id)?);
            filter.insert("update_time", item.update_time);

            let mut doc = Document::new();
            doc.insert("zone_seq", item.seq);
            let doc = doc! {"$set": doc};
            match self.db.coll.find_one_and_update(filter, doc, None).await {
                Ok(Some(_)) => result.push(true),
                Ok(None) => result.push(false),
                Err(e) => {
                    let msg = format!(
                        "update object error: id={}, update_time={}, {}",
                        item.object_id, item.update_time, e
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::MongoDBError, msg));
                }
            }
        }

        Ok(result)
    }
}

#[async_trait]
impl NOCUpdaterProvider for ObjectDBCache {
    async fn try_get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        ObjectDBCache::try_get(&self, object_id).await
    }

    async fn update_signs(&self, req: &ObjectCacheData, insert_time: &u64) -> BuckyResult<usize> {
        ObjectDBCache::update_signs(&self, req, insert_time).await
    }

    async fn replace_old(
        &self,
        req: &ObjectCacheData,
        old: &ObjectCacheData,
    ) -> BuckyResult<usize> {
        ObjectDBCache::replace_old(&self, req, old).await
    }

    async fn insert_new(&self, req: &ObjectCacheData) -> BuckyResult<usize> {
        ObjectDBCache::insert_new(&self, req).await
    }
}

#[async_trait]
impl NamedObjectStorage for ObjectDBCache {
    async fn insert_object(
        &self,
        obj_info: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        self.insert(obj_info, event).await
    }

    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        self.get(object_id).await
    }

    async fn select_object(
        &self,
        filter: &NamedObjectCacheSelectObjectFilter,
        opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        self.select(filter, opt).await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        self.delete(req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.stat().await
    }

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>> {
        Some(Box::new(Clone::clone(&self as &ObjectDBCache)))
    }

    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>> {
        Some(Box::new(Clone::clone(&self as &ObjectDBCache)))
    }

    fn clone(&self) -> Box<dyn NamedObjectStorage> {
        Box::new(Clone::clone(&self as &ObjectDBCache)) as Box<dyn NamedObjectStorage>
    }
}

#[async_trait]
impl NamedObjectCacheSyncServer for ObjectDBCache {
    async fn get_latest_seq(&self) -> BuckyResult<u64> {
        ObjectDBCache::get_latest_seq(self).await
    }

    // 查询指定的同步列表
    async fn list_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        page_index: u16,
        page_size: u16,
    ) -> BuckyResult<Vec<SyncObjectData>> {
        self.list(begin_seq, end_seq, page_index, page_size).await
    }

    async fn get_objects(
        &self,
        begin_seq: u64,
        end_seq: u64,
        list: &Vec<ObjectId>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        ObjectDBCache::get_objects(self, begin_seq, end_seq, list).await
    }
}

#[async_trait]
impl NamedObjectCacheSyncClient for ObjectDBCache {
    async fn query_object(&self, object_id: &ObjectId, update_time: &u64) -> BuckyResult<bool> {
        ObjectDBCache::query_object(&self, object_id, update_time).await
    }

    async fn diff_objects(&self, list: &Vec<SyncObjectData>) -> BuckyResult<Vec<bool>> {
        ObjectDBCache::diff_objects(&self, list).await
    }
}
