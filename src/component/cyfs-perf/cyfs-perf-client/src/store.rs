use crate::isolate;

use super::isolate::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_perf_base::*;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use chrono::{Datelike, Timelike, Utc, DateTime};

// 基于noc的统计项缓存
// 需要注意数据丢失和数据重复的两个核心问题，需要小心处理

#[derive(Clone)]
pub(crate) struct PerfStore {
    locked: Arc<AtomicBool>,

    people_id: ObjectId,
    device_id: ObjectId,

    dec_id: ObjectId,

    id: String,

    span_times: Vec<u32>,

    stack: SharedCyfsStack,
}

impl PerfStore {
    pub fn new(span_time: u32, people_id: ObjectId, device_id: ObjectId, dec_id: Option<ObjectId>, id: impl Into<String>, stack: SharedCyfsStack) -> Self {
        let locked = Arc::new(AtomicBool::new(false));

        let mut span_duration = span_time;
        if span_time < 1 || span_time >= 1440 {
            span_duration = 60;
        }
        let mut span_times = Vec::new();
        let mut seg = 0;
        while seg < 1440 {
            span_times.push(seg);
            seg += span_duration;
        }
        
        let dec_id = match dec_id {
            Some(id) => id,
            None => ObjectId::from_str(PERF_SERVICE_DEC_ID).unwrap(),
        };  
        
        Self {
            locked,
            people_id,
            device_id,
            dec_id,
            id: id.into(),
            stack,
            span_times,

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

    fn get_cur_time_span(&self, date_time: DateTime<Utc>) ->(String, String) {
        let (_is_common_era, year) = date_time.year_ce();
        let date = format!("{:02}-{:02}-{:02}", year, date_time.month(), date_time.day());
        
        //let time_span = format!("{:02}:{:02}", now.hour(), now.minute());
        let cur_span_time = date_time.hour() * 60 + date_time.minute();
        let slot = self.search_lastsmall(self.span_times.to_owned(), cur_span_time);
        let cur_span_time = self.span_times[slot as usize];
        let hour = cur_span_time / 60;
        let minute = cur_span_time % 60;
        let time_span = format!("{:02}:{:02}", hour, minute);

        (date, time_span)
    }

    fn get_local_cache_path(&self, isolate_id: impl Into<String>, id: impl Into<String>, date_span: impl Into<String>, time_span: impl Into<String>, perf_type: PerfType) -> String {
        let people_id = self.people_id.to_string();
        let device_id = self.device_id.to_string();
        let isolate_id = isolate_id.into();
        let id = id.into();
        let date_span = date_span.into();
        let time_span = time_span.into();
        //<owner>/<device>/<isolate_id>/<id>/<PerfType>/<Date>/<TimeSpan>
        let path = format!("/{PERF_SERVICE_DEC_ID}/{people_id}/{device_id}/{isolate_id}/{id}/{perf_type}/{date_span}/{time_span}");

        path
    }

    async fn local_cache(&self, device_id: Option<ObjectId>, dec_id: Option<ObjectId>, 
        isolate_id: impl Into<String>, id: impl Into<String>, date_span: impl Into<String>, time_span: impl Into<String>, 
        perf_object_id: ObjectId, perf_type: PerfType) -> BuckyResult<()>{
        // 把对象存到root_state
        let root_state = self.stack.root_state_stub(device_id, dec_id);
        let op_env = root_state.create_path_op_env().await?;
        let path = self.get_local_cache_path(isolate_id, id, date_span, time_span, perf_type);
        if perf_type == PerfType::Actions {
            op_env.set_with_key(&path, perf_object_id.to_string(), &perf_object_id, None, true).await?;
        } else{
            op_env.set_with_path(&path, &perf_object_id, None, true).await?;
        }
        let root = op_env.commit().await?;
        info!("new dec root is: {:?}, perf_obj_id={}", root, perf_object_id);

        Ok(())
    }

    async fn put_noc_and_root_state(
        &self, 
        object_id: ObjectId, isolate_id: impl Into<String>, id: impl Into<String>, 
        date_span: impl Into<String>, time_span: impl Into<String>, 
        object_raw: Vec<u8>, perf_type: PerfType) -> BuckyResult<()>{
        self.put_object(object_id, object_raw).await?;
        self.local_cache(
            Some(self.device_id), 
            Some(self.dec_id), 
            isolate_id.into(), 
            id.into(),
            date_span.into(),
            time_span.into(),
            object_id, perf_type).await?;

        Ok(())
    }

    async fn get_op_env_object(&self, isolate_id: impl Into<String>, id: impl Into<String>, date_span: impl Into<String>, time_span: impl Into<String>, perf_type: PerfType) -> BuckyResult<Option<ObjectId>> {
        let path = self.get_local_cache_path(isolate_id, id, date_span, time_span, perf_type);
        let root_state = self.stack.root_state_stub(Some(self.device_id), Some(self.dec_id));
        let op_env = root_state.create_path_op_env().await?;
        let ret = op_env.get_by_path(&path).await?;

        Ok(ret)
    }

    // 尝试保存到noc，保存成功后会清空isolates内容
    pub fn save(&self, isolates: &HashMap<String, PerfIsolate>) {
        // 锁定状态下，不可修改数据
        if self.is_locked() {
            warn!("perf store still in locked state!");
            return;
        }

        for (key, isolate) in isolates {
            let data = isolate.take_data();
            let isolate_id = key.to_owned();
            let this = self.clone();
            async_std::task::spawn(async move {
                let _ = this.request(&isolate_id, data.request).await;
                let _ = this.acc(&isolate_id, data.accumulations).await;
                let _ = this.action(&isolate_id, data.actions).await;
                let _ = this.record(&isolate_id, data.records).await;
            });
            info!("will save perf isolate: {}", key);
        }

    }

    // request
    async fn request(&self, isolate_id: &String, request: HashMap<String, Vec<PerfRequestItem>>) -> BuckyResult<()> {
        for (id, items) in request {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfRequestItem>> = HashMap::new();
            for item in items {
                let d = UNIX_EPOCH + Duration::from_secs(item.time);
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(d);
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                match groups.entry(id) {
                    Entry::Vacant(v) => {
                        v.insert(vec![item]);
                    }
                    Entry::Occupied(mut o) => {
                        let v = o.get_mut();       
                        v.push(item);

                    }
                }

            }

            for (key, values) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let ret = self.get_op_env_object(isolate_id, id.to_owned(), date_span, time_span, PerfType::Requests).await?;
                if ret.is_none() {
                    let mut request = PerfRequest::create(self.people_id, self.dec_id);
                    request  = request.add_stats(values.as_slice());
                    
                    let object_raw = request.to_vec()?;
                    let object_id = request.desc().object_id();
                    self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Requests).await?;    
                } else {
                    let v = ret.unwrap();
                    let req = NONGetObjectRequest::new_noc(v, None);
                    if let Ok(resp) = self.stack.non_service().get_object(req).await {
                        let mut request = PerfRequest::decode(&resp.object.object_raw)?;
                        request  = request.add_stats(&values.as_slice());
                
                        let object_raw = request.to_vec()?;
                        let object_id = request.desc().object_id();
                        self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Requests).await?;
                    }
    
                }

            }            
        }

        Ok(())

    }

    // acc
    async fn acc(&self, isolate_id: &String, request: HashMap<String, Vec<PerfAccumulationItem>>) -> BuckyResult<()> {
        for (id, items) in request {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfAccumulationItem>> = HashMap::new();
            for item in items {
                let d = UNIX_EPOCH + Duration::from_secs(item.time);
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(d);
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                match groups.entry(id) {
                    Entry::Vacant(v) => {
                        v.insert(vec![item]);
                    }
                    Entry::Occupied(mut o) => {
                        let v = o.get_mut();       
                        v.push(item);

                    }
                }

            }

            for (key, values) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let ret = self.get_op_env_object(isolate_id, id.to_owned(), date_span, time_span, PerfType::Accumulations).await?;
                if ret.is_none() {
                    let mut request = PerfAccumulation::create(self.people_id, self.dec_id);
                    request  = request.add_stats(values.as_slice());
                    
                    let object_raw = request.to_vec()?;
                    let object_id = request.desc().object_id();
                    self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Accumulations).await?;    
                } else {
                    let v = ret.unwrap();
                    let req = NONGetObjectRequest::new_noc(v, None);
                    if let Ok(resp) = self.stack.non_service().get_object(req).await {
                        let mut request = PerfAccumulation::decode(&resp.object.object_raw)?;
                        request  = request.add_stats(&values.as_slice());
                
                        let object_raw = request.to_vec()?;
                        let object_id = request.desc().object_id();
                        self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Accumulations).await?;
                    }
    
                }

            }            
        }

        Ok(())

    }

    // action
    async fn action(&self, isolate_id: &String, actions: HashMap<String, Vec<PerfActionItem>>) -> BuckyResult<()> {
        for (id, items) in actions {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfActionItem>> = HashMap::new();
            for item in items {
                let d = UNIX_EPOCH + Duration::from_secs(item.time);
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(d);
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                match groups.entry(id) {
                    Entry::Vacant(v) => {
                        v.insert(vec![item]);
                    }
                    Entry::Occupied(mut o) => {
                        let v = o.get_mut();       
                        v.push(item);

                    }
                }

            }

            for (key, stats) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let ret = self.get_op_env_object(isolate_id, id.to_owned(), date_span, time_span, PerfType::Actions).await?;
                if ret.is_none() {
                    let mut action = PerfAction::create(self.people_id, self.dec_id);
                    for stat in stats {
                        action  = action.add_stat(stat);
                    }
                    
                    let object_raw = action.to_vec()?;
                    let object_id = action.desc().object_id();
                    self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Actions).await?;    
                } else {
                    let v = ret.unwrap();
                    let req = NONGetObjectRequest::new_noc(v, None);
                    if let Ok(resp) = self.stack.non_service().get_object(req).await {
                        let mut action = PerfAction::decode(&resp.object.object_raw)?;
                        for stat in stats {
                            action  = action.add_stat(stat);
                        }
                        let object_raw = action.to_vec()?;
                        let object_id = action.desc().object_id();
                        self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Actions).await?;
                    }
    
                }

            }            
        }

        Ok(())

    }

    // record
    async fn record(&self, isolate_id: &String, actions: HashMap<String, Vec<PerfRecordItem>>) -> BuckyResult<()> {
        for (id, items) in actions {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfRecordItem>> = HashMap::new();
            for item in items {
                let d = UNIX_EPOCH + Duration::from_secs(item.time);
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(d);
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                match groups.entry(id) {
                    Entry::Vacant(v) => {
                        v.insert(vec![item]);
                    }
                    Entry::Occupied(mut o) => {
                        let v = o.get_mut();       
                        v.push(item);

                    }
                }

            }

            for (key, stats) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let mut record = PerfRecord::create(self.people_id, self.dec_id, stats[stats.len() - 1].total, stats[stats.len() - 1].total_size);
              
                let object_raw = record.to_vec()?;
                let object_id = record.desc().object_id();
                self.put_noc_and_root_state(object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Records).await?; 

            }            
        }

        Ok(())

    }

    // 锁定区间用以上报操作
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    pub fn lock_for_report(&self) -> bool {
        let ret = self.locked.swap(true, Ordering::SeqCst);
        if !ret {
            info!("lock perf store for reporting!");
        } else {
            error!("perf store already been locked!");
        }

        ret
    }

    pub fn unlock_for_report(&self) {
        let ret = self.locked.swap(false, Ordering::SeqCst);
        if ret {
            info!("unlock perf store after report!");
        } else {
            error!("perf store not been locked yet!");
        }
    }

}
