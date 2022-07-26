use super::perf::*;
use cyfs_base::*;

use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};

pub struct PerfHolder {
    id: String,
    perf: OnceCell<Box<dyn Perf>>,
    children: Mutex<Vec<PerfHolderRef>>,
}

impl PerfHolder {
    pub fn new(id: impl Into<String>) -> PerfHolderRef {
        let ret= Self {
            id: id.into(),
            perf: OnceCell::new(),
            children: Mutex::new(vec![]),
        };

        Arc::new(ret)
    }

    pub fn bind(&self, perf: Box<dyn Perf>) {
        let list = self.children.lock().unwrap();
        list.iter().for_each(|holder| match perf.fork(&holder.id) {
            Ok(perf) => {
                holder.bind(perf);
            }
            Err(e) => {
                error!("fork perf error! id={}, {}", holder.id, e);
            }
        });

        if let Err(_) = self.perf.set(perf) {
            unreachable!();
        }
    }

    pub fn fork(&self, id: impl Into<String>) -> BuckyResult<PerfHolderRef> {
        let new_item = Self::new(id);

        self.children.lock().unwrap().push(new_item.clone());

        if let Some(perf) = self.perf.get() {
            let perf = perf.fork(&new_item.id)?;
            new_item.bind(perf);
        }

        Ok(new_item)
    }

    pub fn get(&self) -> Option<&Box<dyn Perf>> {
        self.perf.get()
    }
}

pub type PerfHolderRef = Arc<PerfHolder>;