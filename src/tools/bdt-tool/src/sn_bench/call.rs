use std::{
    time::Duration, 
};
use async_std::{
    task,
    future,
};

use cyfs_base::*;
use cyfs_bdt::*;

use super::*;

pub async fn sn_bench_call(
    device_num: u64, device_dir: &str,
    sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>, 
    bench_time_sec: u64,
    call_interval_ms: u64,
    timeout_sec: u64,
    _: SnBenchPingException) -> BuckyResult<SnBenchResult> {
    let call_bench_result = SnBenchResult::default();

    println!("init devices");
    let device_emulator = {
        if device_dir.len() != 0 { //load the dir's desc files
            unimplemented!()
        } else { //create device
            let de = device_stack_new(device_num, sns.clone(), endpoints, 3600*24*30).await;
            device_save(de.clone()).await;
            de
        }
    };

    println!("sn online");
    let mut tasks = vec![];
    for i in 0..device_num {
        let de = device_emulator.clone();
        let snsl = sns.clone();
        let stacks = de.0.stacks.lock().unwrap();
        let stack = stacks.get(i as usize).unwrap().clone();
        tasks.push(task::spawn(async move {
            loop {
                let mut online = false;

                match future::timeout(
                    Duration::from_secs(timeout_sec),
                    stack.sn_client().ping().wait_online(),
                ).await {
                    Ok(res) => {
                        match res {
                            Ok(res) => {
                                match res {
                                    SnStatus::Online => {
                                        online = true;
                                    },
                                    _ => {
                                    }
                                }
                            },
                            Err(_) => {
                            }
                        }
                    },
                    Err(_) => {
                    }
                }
                if online {
                    break ;
                }

                if let Some(sns) = snsl.as_ref() {
                    stack.reset_sn_list(sns.clone());
                }
                task::sleep(Duration::from_millis(500)).await;
            }
        }));
    }
    for t in tasks {
        let _ = t.await;
    }

    //
    println!("start benching");
    let exit_flag = Arc::new(RwLock::new(false));
    let mut tasks = vec![];
    let start = bucky_time_now();
    let offset = (device_num/2) as usize;
    for i in 0..offset {
        let flag = exit_flag.clone();
        let de = device_emulator.clone();
        let stacks = de.0.stacks.lock().unwrap();
        let stack = stacks.get(i as usize).unwrap().clone();
        let devices = de.0.devices.clone();
        let result = call_bench_result.clone();
        tasks.push(task::spawn(async move {
            let pinger = cyfs_bdt::debug::Pinger::open(stack.clone().to_weak()).unwrap();
            let idx = i;
            let mut next_remote_idx = idx + offset;
            loop {
                {
                    if *flag.read().unwrap() {
                        break ;
                    }
                }

                let remote = {
                    let dev = devices.read().unwrap();
                    if let Some(d) = dev.get(next_remote_idx) {
                        next_remote_idx += 1;
                        Some(d.clone())
                    } else {
                        None
                    }
                };

                if let Some(remote) = remote {
                    let mut cost = 0;
                    match pinger.ping(remote.clone(), Duration::from_secs(timeout_sec), "debug".as_ref()).await {
                        Ok(rtt) => {
                            match rtt {
                                Some(rtt) => {
                                    match pinger.ping(remote.clone(), Duration::from_secs(timeout_sec), "debug".as_ref()).await {
                                        Ok(rtt2) => {
                                            match rtt2 {
                                                Some(rtt2) => {
                                                    cost = if rtt > rtt2 {
                                                        rtt - rtt2
                                                    } else {
                                                        rtt2 - rtt
                                                    };
                                                },
                                                _ => {
                                                }
                                            }
                                        },
                                        Err(_) => {
                                        }
                                    }
                                },
                                _ => {
                                }
                            }
                        },
                        Err(_) => {
                        }
                    }

                    result.add_resp_time(cost);

                    task::sleep(Duration::from_millis(call_interval_ms)).await;
                } else {
                    next_remote_idx = offset;
                }
            }
        }));
    }
    //wait
    task::sleep(Duration::from_secs(bench_time_sec)).await;
    {
        let mut flag = exit_flag.write().unwrap();
        *flag = true;
    }
    for t in tasks {
        let _ = t.await;
    }
    let end = bucky_time_now();

    //
    call_bench_result.stat(start, end);

    Ok(call_bench_result)
}
