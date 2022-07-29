use super::isolate::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_perf_base::*;

use std::{collections::HashMap};
use chrono::{Datelike, Timelike, Utc, DateTime};

// 基于noc的统计项缓存
// 需要注意数据丢失和数据重复的两个核心问题，需要小心处理

#[derive(Clone)]
pub struct PerfStore {
    people_id: ObjectId,

    dec_id: ObjectId,
    span_times: Vec<u32>,

    stack: SharedCyfsStack,
}

impl PerfStore {
    pub fn new(span_time: u32, people_id: ObjectId, dec_id: ObjectId, stack: SharedCyfsStack) -> Self {

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
        
        Self {
            people_id,
            dec_id,
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
        let dec_id = self.dec_id.to_string();
        let isolate_id = isolate_id.into();
        let id = id.into();
        let date_span = date_span.into();
        let time_span = time_span.into();
        // /local/<dec_id>/<isolate_id>/<id>/<PerfType>/<Date>/<TimeSpan>
        let path = format!("/local/{dec_id}/{isolate_id}/{id}/{perf_type}/{date_span}/{time_span}");

        path
    }

    async fn local_cache(
        &self,
        map: &PathOpEnvStub,
        isolate_id: impl Into<String>,
        id: impl Into<String>, 
        date_span: impl Into<String>, 
        time_span: impl Into<String>, 
        perf_object_id: ObjectId, 
        perf_type: PerfType
        ) -> BuckyResult<()> {
             
        let path = self.get_local_cache_path(isolate_id, id, date_span, time_span, perf_type);
        map.set_with_path(&path, &perf_object_id, None, true).await?;
        // 外部批量处理完, 上层统一commit

        Ok(())
    }

    async fn put_noc_and_root_state(
        &self,
        map: &PathOpEnvStub,
        object_id: ObjectId, 
        isolate_id: impl Into<String>, 
        id: impl Into<String>, 
        date_span: impl Into<String>, 
        time_span: impl Into<String>, 
        object_raw: Vec<u8>, 
        perf_type: PerfType
    ) -> BuckyResult<()>{
        self.put_object(object_id, object_raw).await?;
        
        self.local_cache(
            map,
            isolate_id.into(), 
            id.into(),
            date_span.into(),
            time_span.into(),
            object_id, 
            perf_type
        ).await?;

        Ok(())
    }

    async fn get_op_env_object(
        &self, 
        map: &PathOpEnvStub, 
        isolate_id: impl Into<String>, 
        id: impl Into<String>, 
        date_span: impl Into<String>, 
        time_span: impl Into<String>, 
        perf_type: PerfType
        ) -> BuckyResult<Option<ObjectId>> {
        let path = self.get_local_cache_path(isolate_id, id, date_span, time_span, perf_type);
        let ret = map.get_by_path(path).await?;
        Ok(ret)
    }

    // request
    pub async fn request(&self, map: &PathOpEnvStub, isolate_id: &String, request: HashMap<String, Vec<PerfRequestItem>>) -> BuckyResult<()> {
        for (id, items) in request {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfRequestItem>> = HashMap::new();
            for item in items {
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(bucky_time_to_system_time(item.time));
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                groups.entry(id).or_insert(vec![]).push(item);
            }

            for (key, values) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let ret = self.get_op_env_object(map, isolate_id, id.to_owned(), date_span, time_span, PerfType::Requests).await?;
                if ret.is_none() {
                    let mut request = PerfRequest::create(self.people_id, self.dec_id);
                    request  = request.add_stats(values.as_slice());
                    
                    let object_raw = request.to_vec()?;
                    let object_id = request.desc().object_id();
                    self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Requests).await?;    
                } else {
                    let v = ret.unwrap();
                    let req = NONGetObjectRequest::new_noc(v, None);
                    if let Ok(resp) = self.stack.non_service().get_object(req).await {
                        let mut request = PerfRequest::decode(&resp.object.object_raw)?;
                        request  = request.add_stats(&values.as_slice());
                
                        let object_raw = request.to_vec()?;
                        let object_id = request.desc().object_id();
                        self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Requests).await?;
                    }
    
                }

            }            
        }

        Ok(())

    }

    // acc
   pub async fn acc(&self, map: &PathOpEnvStub, isolate_id: &String, acc: HashMap<String, Vec<PerfAccumulationItem>>) -> BuckyResult<()> {
        for (id, items) in acc {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfAccumulationItem>> = HashMap::new();
            for item in items {
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(bucky_time_to_system_time(item.time));
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                groups.entry(id).or_insert(vec![]).push(item);
            }

            for (key, values) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let ret = self.get_op_env_object(map, isolate_id, id.to_owned(), date_span, time_span, PerfType::Accumulations).await?;
                if ret.is_none() {
                    let mut request = PerfAccumulation::create(self.people_id, self.dec_id);
                    request  = request.add_stats(values.as_slice());
                    
                    let object_raw = request.to_vec()?;
                    let object_id = request.desc().object_id();
                    self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Accumulations).await?;    
                } else {
                    let v = ret.unwrap();
                    let req = NONGetObjectRequest::new_noc(v, None);
                    if let Ok(resp) = self.stack.non_service().get_object(req).await {
                        let mut request = PerfAccumulation::decode(&resp.object.object_raw)?;
                        request  = request.add_stats(&values.as_slice());
                
                        let object_raw = request.to_vec()?;
                        let object_id = request.desc().object_id();
                        self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Accumulations).await?;
                    }
    
                }

            }            
        }

        Ok(())

    }

    // action
    pub async fn action(&self, map: &PathOpEnvStub, isolate_id: &String, actions: HashMap<String, Vec<PerfActionItem>>) -> BuckyResult<()> {
        for (id, items) in actions {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfActionItem>> = HashMap::new();
            for item in items {
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(bucky_time_to_system_time(item.time));
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                groups.entry(id).or_insert(vec![]).push(item);

            }

            for (key, stats) in groups.iter_mut() {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                let ret = self.get_op_env_object(map, isolate_id, id.to_owned(), date_span, time_span, PerfType::Actions).await?;
                if ret.is_none() {
                    let mut action = PerfAction::create(self.people_id, self.dec_id);
                    action  = action.add_stats(stats);
                    let object_raw = action.to_vec()?;
                    let object_id = action.desc().object_id();
                    self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Actions).await?;    
                } else {
                    let v = ret.unwrap();
                    let req = NONGetObjectRequest::new_noc(v, None);
                    if let Ok(resp) = self.stack.non_service().get_object(req).await {
                        let mut action = PerfAction::decode(&resp.object.object_raw)?;
                        action = action.add_stats(stats);
                        let object_raw = action.to_vec()?;
                        let object_id = action.desc().object_id();
                        self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Actions).await?;
                    }
    
                }

            }            
        }

        Ok(())

    }

    // record
    pub async fn record(&self, map: &PathOpEnvStub, isolate_id: &String, records: HashMap<String, Vec<PerfRecordItem>>) -> BuckyResult<()> {
        for (id, items) in records {
            // group by time_span
            let mut groups: HashMap::<String, Vec<PerfRecordItem>> = HashMap::new();
            for item in items {
                // Create DateTime from SystemTime
                let datetime = DateTime::<Utc>::from(bucky_time_to_system_time(item.time));
                let (date, time_span) = self.get_cur_time_span(datetime);
                let id = format!("{date}_{time_span}");
                groups.entry(id).or_insert(vec![]).push(item);
            }

            for (key, stats) in groups {
                let split = key.split("_").collect::<Vec<_>>();
                let date_span = split[0];
                let time_span = split[1];
                // 只取每个time_span的最新一条即可
                let record = PerfRecord::create(self.people_id, self.dec_id, stats[stats.len() - 1].total, stats[stats.len() - 1].total_size);
              
                let object_raw = record.to_vec()?;
                let object_id = record.desc().object_id();
                self.put_noc_and_root_state(map, object_id, isolate_id, id.to_owned(), date_span, time_span, object_raw, PerfType::Records).await?; 

            }            
        }

        Ok(())

    }

}
