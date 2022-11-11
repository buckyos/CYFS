use std::collections::{HashMap};
use std::sync::RwLock;
use crate::bench::*;

pub struct Stat {
    metrics: RwLock<HashMap<String, HashMap<String, Vec<u64>>>>
}

fn print_data(action: &str, data: &mut Vec<u64>) {
    if data.len() == 1 {
        println!("\t\taction {}, use {}ms", action, data[0]);
    } else {
        data.sort();
        let sum: u64 = data.iter().sum();
        println!("\t\taction {}, samples: {}, total: {}ms, avg: {}ms, min: {}ms, max: {}ms", action, data.len(), sum, sum / data.len() as u64, data[0],  data[data.len() - 1]);
    }
}

impl Stat {
    pub(crate) fn new() -> Stat {
        Self {
            metrics: RwLock::new(HashMap::new())
        }
    }
    pub(crate) fn write(&self, case: &str, key: &str, costs: u64) {
        self.metrics.write().unwrap().entry(case.to_owned()).or_insert(HashMap::new()).entry(key.to_owned()).or_insert(vec![]).push(costs);
    }

    pub(crate) fn print(&self, case_list: &[Box<dyn Bench>]) {
        println!("Summary: ");
        for case in case_list {
            if let Some(case_mertics) = self.metrics.write().unwrap().get_mut(case.name()) {
                println!("\ttest case: {}", case.name());
                if let Some(actions) = case.print_list() {
                    for action in actions {
                        if let Some(data) = case_mertics.get_mut(*action) {
                            print_data(action, data)
                        }
                    }
                } else {
                    for (action, data) in case_mertics {
                        print_data(action, data)
                    }
                }

            }
        }

    }
}