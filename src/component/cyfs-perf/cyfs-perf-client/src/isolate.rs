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

    device_id: ObjectId,
    dec_id: ObjectId,

    isolate_id: String,

    id: String,

    span_times: Vec<u32>,

    pending_reqs: HashMap<String, u64>,

    // 本地缓存对象
    request: HashMap<String, PerfRequest>,
    acc: HashMap<String, PerfAccumulation>,
    record: HashMap<String, PerfRecord>,

    action: Vec<PerfAction>,
}

impl PerfIsolateInner {
    pub fn new(isolate_id: &str, span_times: &Vec<u32>, people_id: &ObjectId, device_id: &ObjectId, dec_id: Option<ObjectId>, id: impl Into<String>, stack: SharedCyfsStack) -> PerfIsolateInner {
        let dec_id = match dec_id {
            Some(id) => id,
            None => ObjectId::from_str(PERF_SERVICE_DEC_ID).unwrap(),
        };

        Self {
            people_id: people_id.to_owned(),
            device_id: device_id.to_owned(),
            isolate_id: isolate_id.to_owned(),
            span_times: span_times.to_owned(),
            id: id.into(),
            dec_id,
            stack,
            pending_reqs: HashMap::new(),
            request: HashMap::new(),
            acc: HashMap::new(),
            record: HashMap::new(),
            action: Vec::new(),
        }
    }

    async fn put_object(&self, object_id: ObjectId, object_raw: Vec<u8>) -> BuckyResult<()>{

        let req = NONPutObjectOutputRequest::new_noc(object_id, object_raw);
        self.stack.non_service().put_object(req).await?;

        Ok(())
    }

    fn search_lastsmall<E: PartialOrd>(&self, data: Vec<E>, target: E) -> i32 {
        if data.len() <= 1 {
            return 0;
        }
        let mut l: i32 = 0;
        // 左闭右开区间
        let mut r = data.len() as i32 -1;
        while l <= r {
            let mid = (l + r) / 2;
            if data[mid as usize] <= target {
                if mid == (data.len() -1) as i32 || data[mid as usize + 1] > target {
                    return mid;
                }
                l = mid + 1;
            } else {
                r = mid - 1;
            }
        }

        return 0;
    }
    

    fn get_local_cache_path(&self, isolate_id: String, id: String, perf_type: PerfType) -> String {
        let now = Utc::now();
        let (_is_common_era, year) = now.year_ce();
        let date = format!("{:02}-{:02}-{:02}", year, now.month(), now.day());
        
        //let time_span = format!("{:02}:{:02}", now.hour(), now.minute());
        let cur_span_time = now.hour() * 60 + now.minute();
        let slot = self.search_lastsmall(self.span_times.to_owned(), cur_span_time);
        let cur_span_time = self.span_times[slot as usize];
        let hour = cur_span_time / 60;
        let minute = cur_span_time % 60;
        let time_span = format!("{:02}:{:02}", hour, minute);
        let people_id = self.people_id.to_string();
        let device_id = self.device_id.to_string();
        //<owner>/<device>/<isolate_id>/<id>/<PerfType>/<Date>/<TimeSpan>
        let path = format!("/{PERF_SERVICE_DEC_ID}/{people_id}/{device_id}/{isolate_id}/{id}/{perf_type}/{date}/{time_span}");

        path
    }

    async fn local_cache(&self, device_id: Option<ObjectId>, dec_id: Option<ObjectId>, isolate_id: String, id: String, perf_object_id: ObjectId, perf_type: PerfType) -> BuckyResult<()>{
        // 把对象存到root_state
        let root_state = self.stack.root_state_stub(device_id, dec_id);
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

    async fn put_noc_and_root_state(&self, object_id: ObjectId, id: String, object_raw: Vec<u8>, perf_type: PerfType) -> BuckyResult<()>{
        let _ = self.put_object(object_id, object_raw).await;
        let _ = self.local_cache(
            Some(self.device_id), 
            Some(self.dec_id), 
            self.isolate_id.to_owned(), 
            id, 
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

    pub fn end_request(&mut self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        let now = bucky_time_now();
        let full_id = format!("{}_{}", id, key);
        match self.pending_reqs.remove(&full_id) {
            Some(tick) => {
                let during = if now > tick {
                    now - tick
                } else {
                    0
                };

                match self.request.entry(id.to_owned()) {
                    Entry::Vacant(v) => {
                        let perf_obj = PerfRequest::create(self.people_id, self.dec_id);
                        let req = perf_obj.add_stat(during, err, bytes);

                        v.insert(req);
                    }
                    Entry::Occupied(mut o) => {
                        let perf_obj = o.get_mut();
                        let req = perf_obj.add_stat(during, err, bytes);
                        
                        o.insert(req);

                    }
                }
            }
            None => {
                //unreachable!();
            }
        }

        // let full_id = format!("{}_{}", id, key);

        // let path = self.get_local_cache_path(self.isolate_id.to_owned(), id.to_string(), PerfType::Requests);
        // let root_state = self.stack.root_state_stub(Some(self.device_id), Some(self.dec_id));
        // let op_env = root_state.create_path_op_env().await?;
        // let ret = op_env.get_by_path(&path).await?;
        // if ret.is_none() {
        //     match self.pending_reqs.remove(&full_id) {
        //         Some(_tick) => {
        //             let perf_obj = PerfRequest::create(self.people_id, self.dec_id);
        //             let v = perf_obj.add_stat(spend_time, stat);
        //             //FIXME: 异步保存数据
        //             let object_raw = v.to_vec()?;
        //             let object_id = v.desc().object_id();
        //             self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Requests).await?
        //         }
        //         None => {
        //             unreachable!();
        //         }
        //     }
        //     return;
        // }
        // let v = ret.unwrap();
        // let req = NONGetObjectRequest::new_noc(v, None);
        // match self.stack.non_service().get_object(req).await {
        //     Ok(resp) => {
        //         let perf_obj = PerfRequest::decode(&resp.object.object_raw)?;
        //         match self.pending_reqs.remove(&full_id) {
        //             Some(_tick) => {
        //                 let v = perf_obj.add_stat(spend_time, stat);
        //                 //FIXME: 异步保存数据
        //                 let object_raw = v.to_vec()?;
        //                 let object_id = v.desc().object_id();
        //                 self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Requests).await?
        //             }
        //             None => {
        //                 unreachable!();
        //             }
        //         }
        //     },
        //     Err(_) => {
        //         match self.pending_reqs.remove(&full_id) {
        //             Some(_tick) => {
        //                 let perf_obj = PerfRequest::create(self.people_id, self.dec_id);
        //                 let v = perf_obj.add_stat(spend_time, stat);
        //                 //FIXME: 异步保存数据
        //                 let object_raw = v.to_vec()?;
        //                 let object_id = v.desc().object_id();
        //                 self.put_noc_and_root_state(object_id,   id.to_string(), object_raw, PerfType::Requests).await?
        //             }
        //             None => {
        //                 unreachable!();
        //             }
        //         }
        //     },
        // }

    }

    pub fn acc(&mut self, id: &str, err: BuckyErrorCode, size: Option<u64>) {

        // let path = self.get_local_cache_path(self.isolate_id.to_owned(), id.to_owned(), PerfType::Accumulations);

        // let root_state = self.stack.root_state_stub(Some(self.device_id), Some(self.dec_id));
        // let op_env = root_state.create_path_op_env().await?;
        // let ret = op_env.get_by_path(&path).await?;
        // if ret.is_none() {
        //     let perf_obj = PerfAccumulation::create(self.people_id, self.dec_id);
        //     let v = perf_obj.add_stat(stat);
        //     // FIXME: 异步保存数据
        //     let object_raw = v.to_vec()?;
        //     let object_id = v.desc().object_id();
        //     self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Accumulations).await?;
        //     return Ok(());
        // }
        // let v = ret.unwrap();
        // let req = NONGetObjectRequest::new_noc(v, None);
        // match self.stack.non_service().get_object(req).await{
        //     Ok(resp) => {
        //         let perf_obj = PerfAccumulation::decode(&resp.object.object_raw)?;
        //         let v = perf_obj.add_stat(stat);
        //         // FIXME: 异步保存数据
        //         let object_raw = v.to_vec()?;
        //         let object_id = v.desc().object_id();
        //         self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Accumulations).await?;
        //     },
        //     Err(_) => {
        //         let perf_obj = PerfAccumulation::create(self.people_id, self.dec_id);
        //         let v = perf_obj.add_stat(stat);
        //         // FIXME: 异步保存数据
        //         let object_raw = v.to_vec()?;
        //         let object_id = v.desc().object_id();
        //         self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Accumulations).await?;
        //     },
        // }


    }

    pub fn action(
        &mut self,
        id: &str,
        err: BuckyErrorCode,
        name: impl Into<String>,
        value: impl Into<String>,
    ) {
        // FIXME: 本地缓存, 异步写操作, 默认10分钟

        // let v = PerfAction::create(self.people_id, self.dec_id, stat);
        // let object_raw = v.to_vec()?;
        // let object_id = v.desc().object_id();
        // self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Actions).await?;

    }

    pub async fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        // let path = self.get_local_cache_path(self.isolate_id.to_owned(), self.id.to_owned(), PerfType::Records);

        // let root_state = self.stack.root_state_stub(Some(self.device_id), Some(self.dec_id));
        // let op_env = root_state.create_path_op_env().await?;
        // let ret = op_env.get_by_path(&path).await?;
        // if ret.is_none() {
        //     let perf_obj = PerfRecord::create(self.people_id, self.dec_id, total, total_size);
        //     let v = perf_obj.add_stat(total, total_size);
        //     // FIXME: 异步保存数据
        //     let object_raw = v.to_vec()?;
        //     let object_id = v.desc().object_id();
        //     self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Records).await?;
        //     return ;
        // }
        // let v = ret.unwrap();
        // let req = NONGetObjectRequest::new_noc(v, None);
        // match self.stack.non_service().get_object(req).await{
        //     Ok(resp) => {
        //         let perf_obj = PerfRecord::decode(&resp.object.object_raw)?;
        //         let v = perf_obj.add_stat(total, total_size);
        //         // FIXME: 异步保存数据
        //         let object_raw = v.to_vec()?;
        //         let object_id = v.desc().object_id();
        //         self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Records).await?;
        //     },
        //     Err(_) => {
        //         let perf_obj = PerfRecord::create(self.people_id, self.dec_id, total, total_size);
        //         let v = perf_obj.add_stat(total, total_size);
        //         // FIXME: 异步保存数据
        //         let object_raw = v.to_vec()?;
        //         let object_id = v.desc().object_id();
        //         self.put_noc_and_root_state(object_id, id.to_string(), object_raw, PerfType::Records).await?;
        //     },
        // }
    }

}


#[derive(Clone)]
pub struct PerfIsolate(Arc<Mutex<PerfIsolateInner>>);

impl PerfIsolate {
    pub fn new(isolate_id: &str, span_times: &Vec<u32>, people_id: &ObjectId, device_id: &ObjectId, dec_id: Option<ObjectId>, id: impl Into<String>, stack: SharedCyfsStack) -> Self {
        Self(Arc::new(Mutex::new(PerfIsolateInner::new(isolate_id, span_times, people_id, device_id, dec_id, id, stack))))
    }

    // 开启一个request
    pub fn begin_request(&self, id: &str, key: &str) {
        self.0.lock().unwrap().begin_request(id, key)
    }
    // 统计一个操作的耗时, 流量统计
    pub fn end_request(&self, id: &str, key: &str, err: BuckyErrorCode, bytes: Option<u32>) {
        self.0.lock().unwrap().end_request(id, key, err, bytes)
    }

    pub fn acc(&self, id: &str, err: BuckyErrorCode, size: Option<u64>) {
        self.0.lock().unwrap().acc(id, err, size)
    }

    pub fn action(
        &self,
        id: &str,
        err: BuckyErrorCode,
        name: impl Into<String>,
        value: impl Into<String>,
    ){
        self.0.lock().unwrap().action(id, err, name, value)
    }

    pub fn record(&self, id: &str, total: u64, total_size: Option<u64>) {
        self.0.lock().unwrap().record(id, total, total_size)
    }

    pub fn get_id(&self) -> String {
        self.0.lock().unwrap().id.clone()
    }
}