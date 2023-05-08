use std::{
    sync::{
        Arc,
        Mutex,
        RwLock,
    }
};
use async_std::{
    task,
};

use cyfs_base::*;
use cyfs_bdt::*;

mod ping;
mod call;
mod result;
mod exception;

pub use ping::*;
pub use call::*;
pub use result::*;
pub use exception::*;

pub fn create_device(sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>) -> (Device, PrivateKey) {
    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let public_key = private_key.public();

    let sn_list = match sns.as_ref() {
        Some(sns) => {
            let mut sn_list = Vec::new();
            for sn in sns.iter() {
                sn_list.push(sn.desc().device_id());
            }
            sn_list
        },
        None => vec![],
    };

    let device = Device::new(
        None,
        UniqueId::default(),
        endpoints.clone(),
        sn_list,
        vec![],
        public_key,
        Area::default(), 
        DeviceCategory::PC
    ).build();

    (device, private_key)
}

pub fn new_endpoint(ep: &Endpoint, index: u16) -> Endpoint {
    let mut endpoint = Endpoint::default();
    endpoint.clone_from(ep);

    let addr = endpoint.mut_addr();
    let port = ep.addr().port()+index;
    addr.set_port(port);

    endpoint
}

pub fn device_stack_load() {
}

pub struct DeviceEmulatorImpl {
    stacks: Arc<Mutex<Vec<StackGuard>>>,
    devices: Arc<RwLock<Vec<Device>>>,
}

#[derive(Clone)]
pub struct DeviceEmulator(Arc<DeviceEmulatorImpl>);

pub async fn device_stack_new(device_num: u64, sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>) -> DeviceEmulator {
    let endpoint = endpoints.get(0).unwrap();

    let stacks = Arc::new(Mutex::new(vec![]));
    let devices = Arc::new(RwLock::new(vec![]));

    let stacks_counter = Arc::new(Mutex::new(0));
    let mut tasks = vec![];

    for i in 0..8 {
        let s = stacks.clone();
        let sc = stacks_counter.clone();
        let ept = endpoint.clone();
        let snsl = sns.clone();
        let devs = devices.clone();
        tasks.push(task::spawn(async move {
            loop {
                let idx = {
                    let mut st = sc.lock().unwrap();
                    if *st >= device_num as i32 {
                        -1
                    } else {
                        let tmp = *st;
                        *st = tmp + 1;
                        tmp
                    }
                };
                if idx < 0 {
                    return ;
                }

                let mut endpoints = vec![];

                let ep = new_endpoint(&ept, idx as u16);

                endpoints.push(ep);

                let (device, private_key) = create_device(snsl.clone(), endpoints);

                {
                    let mut d = devs.write().unwrap();
                    d.push(device.clone());
                }

                let params = StackOpenParams::new(format!("bench_sn_{}", i).as_str());
                //params.config.sn_client.ping.interval = Duration::from_secs(1);
                let stack = Stack::open(
                    device, 
                    private_key, 
                    params).await.unwrap();

                if let Some(sns) = snsl.as_ref() {
                    stack.reset_sn_list(sns.clone());
                    let sn_dev = sns.get(0).unwrap().clone();
                    let sn_device_id = sn_dev.desc().device_id();
                    stack.device_cache().add(&sn_device_id, &sn_dev);
                }

                {
                    let mut st = s.lock().unwrap();
                    st.push(stack);
                }
            }
        }));
    }

    for t in tasks {
        let _ = t.await;
    }

    DeviceEmulator(Arc::new(DeviceEmulatorImpl {
        stacks,
        devices,
    }))
}

pub async fn device_save(_: DeviceEmulator) {

}
