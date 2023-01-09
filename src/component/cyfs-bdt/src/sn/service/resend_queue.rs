use log::*;
use std::{
    time::Duration, 
    collections::HashMap, 
    sync::{Arc}
};
use cyfs_debug::Mutex;
use futures::executor::ThreadPool;
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{DynamicPackage, PackageBox}
};

use super::{
    net_listener::{UdpSender}
};


struct PackageResendInfo {
    pkg: Arc<PackageBox>,
    sender: Arc<UdpSender>,
    interval: Duration,
    times: u8,
    last_time: Timestamp,
    nick_name: String,
}

pub trait ResendCallbackTrait: Send + Sync {
    fn on_callback(&self, pkg: Arc<PackageBox>, errno: BuckyErrorCode);
}

pub struct ResendQueue {
    default_interval: Duration,
    max_times: u8,
    thread_pool: ThreadPool, 
    
    packages: Mutex<HashMap<u32, PackageResendInfo>>, 

    cb: Box<dyn ResendCallbackTrait>,

}

impl ResendQueue {
    pub fn new(
        thread_pool: ThreadPool, 
        default_interval: Duration, 
        max_times: u8,
        cb: Box<dyn ResendCallbackTrait>) -> ResendQueue {
        ResendQueue {
            default_interval,
            max_times,
            thread_pool, 
            packages: Mutex::new(Default::default()), 
            cb,
        }
    }

    pub fn send(
        &self, 
        sender: Arc<UdpSender>, 
        pkg: DynamicPackage, 
        pkg_id: u32, 
        pkg_nick_name: String) {
        let now = bucky_time_now();
        let to_send = {
            let mut packages = self.packages.lock().unwrap();
            if let Some(info) = packages.get_mut(&pkg_id) {
                if info.times > 1 {
                    // 短时间内多次send，在前次基础上适当多分配resend机会，避免因为前一次超时导致本次丢失
                    info.times >>= 1;
                    info.interval = info.interval / 2;
                }
                let pkg_box = sender.box_pkg(pkg);
                info.sender = sender;
                info.pkg = Arc::new(pkg_box);
                info.nick_name = pkg_nick_name.clone();
    
                if now > info.last_time 
                    && Duration::from_micros(now - info.last_time) > info.interval {
                    info.times += 1;
                    info.last_time = now;
                    Some((info.sender.clone(), info.pkg.clone()))
                } else {
                    None
                }
            } else {
                let pkg_box = Arc::new(sender.box_pkg(pkg));
                packages.insert(pkg_id, PackageResendInfo {
                    pkg: pkg_box.clone(),
                    sender: sender.clone(),
                    interval: self.default_interval,
                    times: 1,
                    last_time: bucky_time_now(),
                    nick_name: pkg_nick_name.clone(),
                });
                Some((sender, pkg_box.clone()))
            }
        };

        if let Some((sender, pkg)) = to_send {
            self.thread_pool.spawn_ok(async move {
                match sender.send(&*pkg).await {
                    Ok(_) => {
                        info!("{} send ok.", pkg_nick_name);
                    },
                    Err(e) => {
                        warn!("{} send failed, error: {}.", pkg_nick_name, e.to_string());
                    }
                }
            });
        }
    }

    pub fn confirm_pkg(&self, pkg_id: u32) {
        if let Some(will_remove) = self.packages.lock().unwrap().remove(&pkg_id) {
            self.cb.on_callback(will_remove.pkg.clone(), BuckyErrorCode::Ok);
        }
    }

    pub fn try_resend(&self, now: Timestamp) {
        let mut to_send = vec![];
        let mut will_remove = vec![];
        
        {
            let mut packages = self.packages.lock().unwrap();
            for (pkg_id, pkg_info) in packages.iter_mut() {
                if now > pkg_info.last_time 
                    && Duration::from_micros(now - pkg_info.last_time) > pkg_info.interval {
                    pkg_info.times += 1;
                    pkg_info.interval = pkg_info.interval * 2;
                    pkg_info.last_time = now;
                    if pkg_info.times >= self.max_times {
                        will_remove.push(*pkg_id);
                    }
    
                    let pkg = pkg_info.pkg.clone();
                    let sender = pkg_info.sender.clone();
                    let nick_name = pkg_info.nick_name.clone();
    
                    to_send.push((pkg, sender, nick_name));
                }
            }
    
            for id in will_remove {
                let pkg = packages.remove(&id);
                if let Some(p) = pkg {
                    warn!("{} resend timeout, to: {}.", p.nick_name, p.sender.session_name());
                    self.cb.on_callback(p.pkg.clone(), BuckyErrorCode::Timeout);
                }
            }
        }

        for (pkg, sender, nick_name) in to_send {
            self.thread_pool.spawn_ok(async move {
                match sender.send(&*pkg).await {
                    Ok(_) => {
                        info!("{} send ok, to: {}.", nick_name, sender.session_name());
                    },
                    Err(e) => {
                        warn!("{} send failed, to: {}, error: {}.", nick_name, sender.session_name(), e.to_string());
                    }
                }
            });
        }
    }
}
