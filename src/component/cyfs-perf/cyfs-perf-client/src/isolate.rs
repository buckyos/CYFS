use cyfs_base::*;
use cyfs_lib::*;
use cyfs_debug::Mutex;
use cyfs_perf_base::*;

use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use chrono::{Datelike, Timelike, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfType {
    Requests,
    Accumulations,
    Actions,
    Records,

}

impl fmt::Display for PerfType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PerfType::Requests => write!(f, "Requests"),
            PerfType::Accumulations => write!(f, "Accumulations"),
            PerfType::Actions => write!(f, "Actions"),
            PerfType::Records => write!(f, "Records"),
        }
    }
}

#[derive(Clone)]
struct PerfIsolateInner {
    stack: SharedCyfsStack,

    people_id: ObjectId,

    device_id: DeviceId,
    dec_id: ObjectId,

    isolate_id: String,

    id: String,

    actions: Vec<PerfAction>,

    records: HashMap<String, PerfRecord>,

    accumulations: HashMap<String, PerfAccumulation>,

    pending_reqs: HashMap<String, u64>,
    reqs: HashMap<String, PerfRequest>,
}

impl PerfIsolateInner {
    pub fn new(isolate_id: &str, people_id: ObjectId, device_id: DeviceId, dec_id: Option<ObjectId>, id: String, stack: SharedCyfsStack) -> PerfIsolateInner {
        let dec_id = match dec_id {
            Some(id) => id,
            None => ObjectId::from_str(PERF_SERVICE_DEC_ID).unwrap(),
        };

        Self {
            people_id,
            device_id,
            isolate_id: isolate_id.to_owned(),
            id,
            actions: vec![],
            records: HashMap::new(),
            accumulations: HashMap::new(),
            pending_reqs: HashMap::new(),
            reqs: HashMap::new(),
            dec_id,
            stack,
        }
    }

    async fn put_object(&self, object_id: ObjectId, object_raw: Vec<u8>) -> BuckyResult<()>{

        let req = NONPutObjectOutputRequest::new_noc(object_id, object_raw);
        self.stack.non_service().put_object(req).await?;

        Ok(())
    }


    fn get_local_cache_path(&self, isolate_id: String, id: String, perf_type: PerfType) -> String {
        let now = Utc::now();
        let (_is_common_era, year) = now.year_ce();
        let date = format!("{:02}-{:02}-{:02}", year, now.month(), now.day());
        //let time_span = format!("{:02}:{:02}", now.hour(), now.minute());
        let time_span = format!("{:02}:00", now.hour());
        let people_id = self.people_id.to_string();
        let device_id = self.device_id.to_string();
        let dec_id = self.dec_id.to_string();
        // /<owner>/<device>/<DecId>/<isolate_id>/<id>/<PerfType>/<Date>/<TimeSpan>
        let path = format!("/{PERF_SERVICE_DEC_ID}/{people_id}/{device_id}/{dec_id}/{isolate_id}/{id}/{perf_type}/{date}/{time_span}");

        path
    }

    async fn noc_root_state(&self, people_id: Option<ObjectId>, dec_id: Option<ObjectId>, isolate_id: String, id: String, perf_object_id: ObjectId, perf_type: PerfType) -> BuckyResult<()>{
        // 把对象存到root_state
        let root_state = self.stack.root_state_stub(people_id, dec_id);
        let op_env = root_state.create_path_op_env().await?;
        let path = self.get_local_cache_path(isolate_id, id, perf_type);
        if perf_type == PerfType::Actions {
            op_env.set_with_key(&path, perf_object_id.to_string(), &perf_object_id, None, true).await?;
        } else{
            op_env.set_with_path(&path, &perf_object_id, None, true).await?;
        }
        let root = op_env.commit().await?;
        info!("new dec root is: {:?}, perf_obj_id={}", root, perf_object_id);

        Ok(())
    }

    async fn put_noc_and_root_state(&self, object_id: ObjectId, object_raw: Vec<u8>, perf_type: PerfType) -> BuckyResult<()>{
        let _ = self.put_object(object_id, object_raw).await;
        let _ = self.noc_root_state(
            Some(self.people_id), 
            Some(self.dec_id), 
            self.isolate_id.to_owned(), 
            self.id.to_owned(), 
            object_id, perf_type).await;

        Ok(())
    }

    pub fn begin_request(&mut self, id: &str, key: &str) {
        let full_id = format!("{}_{}", id, key);

        match self.pending_reqs.entry(full_id) {
            Entry::Vacant(v) => {
                let bucky_time = bucky_time_now();
                v.insert(bucky_time);
            }
            Entry::Occupied(_o) => {
                //unreachable!("perf request item already begin! id={}, key={}", id, key);
            }
        }
    }

    pub async fn end_request(&mut self, id: &str, key: &str, spend_time: u64, stat: BuckyResult<Option<u64>>) -> BuckyResult<()>{
        let full_id = format!("{}_{}", id, key);

        let path = self.get_local_cache_path(self.isolate_id.to_owned(), self.id.to_owned(), PerfType::Requests);

        let root_state = self.stack.root_state_stub(Some(self.people_id), Some(self.dec_id));
        let op_env = root_state.create_path_op_env().await?;
        let ret = op_env.get_by_path(&path).await?;
        if ret.is_none() {
            match self.pending_reqs.remove(&full_id) {
                Some(_tick) => {
                    let perf_obj = PerfRequest::create(self.people_id, self.dec_id);
                    let v = perf_obj.add_stat(spend_time, stat);
                    let object_raw = v.to_vec()?;
                    let object_id = v.desc().object_id();
                    self.put_noc_and_root_state(object_id, object_raw, PerfType::Requests).await?
                }
                None => {
                    unreachable!();
                }
            }
            return Ok(());
        }
        let v = ret.unwrap();
        let req = NONGetObjectRequest::new_noc(v, None);
        match self.stack.non_service().get_object(req).await {
            Ok(resp) => {
                let perf_obj = PerfRequest::decode(&resp.object.object_raw)?;
                match self.pending_reqs.remove(&full_id) {
                    Some(_tick) => {
                        let v = perf_obj.add_stat(spend_time, stat);
                        let object_raw = v.to_vec()?;
                        let object_id = v.desc().object_id();
                        self.put_noc_and_root_state(object_id, object_raw, PerfType::Requests).await?
                    }
                    None => {
                        unreachable!();
                    }
                }
            },
            Err(_) => {
                match self.pending_reqs.remove(&full_id) {
                    Some(_tick) => {
                        let perf_obj = PerfRequest::create(self.people_id, self.dec_id);
                        let v = perf_obj.add_stat(spend_time, stat);
                        let object_raw = v.to_vec()?;
                        let object_id = v.desc().object_id();
                        self.put_noc_and_root_state(object_id, object_raw, PerfType::Requests).await?
                    }
                    None => {
                        unreachable!();
                    }
                }
            },
        }


        Ok(())
    }

    pub async fn acc(&mut self, _id: &str, stat: BuckyResult<u64>) -> BuckyResult<()>{

        let path = self.get_local_cache_path(self.isolate_id.to_owned(), self.id.to_owned(), PerfType::Accumulations);

        let root_state = self.stack.root_state_stub(Some(self.people_id), Some(self.dec_id));
        let op_env = root_state.create_path_op_env().await?;
        let ret = op_env.get_by_path(&path).await?;
        if ret.is_none() {
            let perf_obj = PerfAccumulation::create(self.people_id, self.dec_id);
            let v = perf_obj.add_stat(stat);
            let object_raw = v.to_vec()?;
            let object_id = v.desc().object_id();
            self.put_noc_and_root_state(object_id, object_raw, PerfType::Accumulations).await?;
            return Ok(());
        }
        let v = ret.unwrap();
        let req = NONGetObjectRequest::new_noc(v, None);
        match self.stack.non_service().get_object(req).await{
            Ok(resp) => {
                let perf_obj = PerfAccumulation::decode(&resp.object.object_raw)?;
                let v = perf_obj.add_stat(stat);
                let object_raw = v.to_vec()?;
                let object_id = v.desc().object_id();
                self.put_noc_and_root_state(object_id, object_raw, PerfType::Accumulations).await?;
            },
            Err(_) => {
                let perf_obj = PerfAccumulation::create(self.people_id, self.dec_id);
                let v = perf_obj.add_stat(stat);
                let object_raw = v.to_vec()?;
                let object_id = v.desc().object_id();
                self.put_noc_and_root_state(object_id, object_raw, PerfType::Accumulations).await?;
            },
        }



        Ok(())
    }

    pub async fn action(
        &mut self,
        _id: &str,
        stat: BuckyResult<(String, String)>,
    ) -> BuckyResult<()>{
        let v = PerfAction::create(self.people_id, self.dec_id, stat);
        let object_raw = v.to_vec()?;
        let object_id = v.desc().object_id();
        self.put_noc_and_root_state(object_id, object_raw, PerfType::Actions).await?;

        Ok(())
    }

    pub async fn record(&mut self, _id: &str, total: u64, total_size: Option<u64>) -> BuckyResult<()>{
        let path = self.get_local_cache_path(self.isolate_id.to_owned(), self.id.to_owned(), PerfType::Records);

        let root_state = self.stack.root_state_stub(Some(self.people_id), Some(self.dec_id));
        let op_env = root_state.create_path_op_env().await?;
        let ret = op_env.get_by_path(&path).await?;
        if ret.is_none() {
            info!("record get_by_path: {path}  not found");
            let perf_obj = PerfRecord::create(self.people_id, self.dec_id, total, total_size);
            let v = perf_obj.add_stat(total, total_size);
            let object_raw = v.to_vec()?;
            let object_id = v.desc().object_id();
            self.put_noc_and_root_state(object_id, object_raw, PerfType::Records).await?;
            return Ok(());
        }
        let v = ret.unwrap();
        let req = NONGetObjectRequest::new_noc(v, None);
        match self.stack.non_service().get_object(req).await{
            Ok(resp) => {
                let perf_obj = PerfRecord::decode(&resp.object.object_raw)?;
                let v = perf_obj.add_stat(total, total_size);
                let object_raw = v.to_vec()?;
                let object_id = v.desc().object_id();
                self.put_noc_and_root_state(object_id, object_raw, PerfType::Records).await?;
            },
            Err(_) => {
                let perf_obj = PerfRecord::create(self.people_id, self.dec_id, total, total_size);
                let v = perf_obj.add_stat(total, total_size);
                let object_raw = v.to_vec()?;
                let object_id = v.desc().object_id();
                self.put_noc_and_root_state(object_id, object_raw, PerfType::Records).await?;
            },
        }


        Ok(())
    }

}


#[derive(Clone)]
pub struct PerfIsolate(Arc<Mutex<PerfIsolateInner>>);

impl PerfIsolate {
    pub fn new(isolate_id: &str, people_id: ObjectId, device_id: DeviceId, dec_id: Option<ObjectId>, id: String, stack: SharedCyfsStack) -> Self {
        Self(Arc::new(Mutex::new(PerfIsolateInner::new(isolate_id, people_id, device_id, dec_id, id, stack))))
    }

    // 开启一个request
    pub fn begin_request(&self, id: &str, key: &str) {
        self.0.lock().unwrap().begin_request(id, key)
    }
    // 统计一个操作的耗时, 流量统计
    pub fn end_request(&self, id: &str, key: &str, spend_time: u64, stat: BuckyResult<Option<u64>>) -> BuckyResult<()> {
        async_std::task::block_on(async { self.0.lock().unwrap().end_request(id, key, spend_time, stat).await })
    }

    pub fn acc(&self, id: &str, stat: BuckyResult<u64>) -> BuckyResult<()> {
        async_std::task::block_on(async { self.0.lock().unwrap().acc(id, stat).await })
    }

    pub fn action(
        &self,
        id: &str,
        stat: BuckyResult<(String, String)>,
    )-> BuckyResult<()> {
        async_std::task::block_on(async { self.0.lock().unwrap().action(id, stat).await })   
    }

    pub fn record(&self, id: &str, total: u64, total_size: Option<u64>) -> BuckyResult<()>{
        async_std::task::block_on(async { self.0.lock().unwrap().record(id, total, total_size).await })
    }

    pub fn get_id(&self) -> String {
        self.0.lock().unwrap().id.clone()
    }
}
