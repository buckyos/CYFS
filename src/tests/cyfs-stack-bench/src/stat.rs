use std::collections::HashMap;
use std::sync::RwLock;
use crate::bench::*;

pub struct Stat {
    metrics: RwLock<HashMap<String, Vec<u64>>>
}



impl Stat {
    pub fn new() -> Stat {
        Self {
            metrics: RwLock::new(HashMap::new())
        }
    }
    pub fn write(&self, key: &str, costs: u64) {
        self.metrics.write().unwrap().entry(key.to_owned()).or_insert(vec![]).push(costs);
    }

    pub fn print(&self) {
        println!("Summary: ");
        let arr = STAT_METRICS_LIST;
        for key in arr.into_iter() {
            if let Some(data) = self.metrics.write().unwrap().get_mut(key) {
                if data.len() == 1 {
                    println!("test: {}, use {}ms", key, data[0]);
                } else {
                    data.sort();
                    let sum: u64 = data.iter().sum();
                    println!("test: {}, samples: {}, total: {}ms, avg: {}ms, min: {}ms, max: {}ms", key, data.len(), sum, sum / data.len() as u64, data[0],  data[data.len() - 1]);
                }
            }
        }
    }
}