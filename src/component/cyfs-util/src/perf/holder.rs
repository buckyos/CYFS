use super::perf::*;
use cyfs_base::*;

use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};

pub struct PerfHolderInner {
    id: Option<String>,
    perf: OnceCell<PerfRef>,
    children: Mutex<Vec<PerfHolder>>,
}

impl PerfHolderInner {
    pub fn new_isolate(id: impl Into<String>) -> PerfHolderInner {
        Self {
            id: Some(id.into()),
            perf: OnceCell::new(),
            children: Mutex::new(vec![]),
        }
    }

    pub fn new() -> PerfHolderInner {
        Self {
            id: None,
            perf: OnceCell::new(),
            children: Mutex::new(vec![]),
        }
    }

    pub fn bind_raw(&self, parent: &PerfRef) {
        let perf = match &self.id {
            Some(id) => {
                match parent.fork(&id) {
                    Ok(perf) => {
                        Arc::new(perf)
                    }
                    Err(e) => {
                        error!("fork perf error! id={}, {}", id, e);
                        return;
                    }
                }
            }
            None => {
                parent.clone()
            }
        };

        let list = self.children.lock().unwrap();
        list.iter().for_each(|holder| holder.bind_raw(&perf)); 

        if let Err(_) = self.perf.set(perf) {
            unreachable!();
        }
    }

   
    pub fn add_child(&self, child_perf: &PerfHolder) {
        self.children.lock().unwrap().push(child_perf.clone());

        if let Some(perf) = self.perf.get() {
            child_perf.bind_raw(perf);
        }
    }

    pub fn get(&self) -> Option<&PerfRef> {
        self.perf.get()
    }
}

#[derive(Clone)]
pub struct PerfHolder(Arc<PerfHolderInner>);


impl PerfHolder {
    pub fn new_isolate(id: impl Into<String>) -> PerfHolder {
        Self(Arc::new(PerfHolderInner::new_isolate(id)))
    }

    pub fn new() -> PerfHolder {
        Self(Arc::new(PerfHolderInner::new()))
    }

    pub fn get(&self) -> Option<&PerfRef> {
        self.0.get()
    }

    pub fn fork(&self, id: impl Into<String>) -> BuckyResult<Self> {
        let new_item = Self::new_isolate(id);

        self.0.add_child(&new_item);

        Ok(new_item)
    }

    pub fn bind(&self, parent: &Self) {
        parent.0.add_child(&self)
    }

    pub fn bind_raw(&self, parent: &PerfRef) {
        self.0.bind_raw(parent)
    }
}