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

async fn stack_sn_online(stack: StackGuard, snsl: Option<Vec<Device>>, timeout_sec: u64) {
    loop {
        if let Some(sns) = snsl.as_ref() {
            stack.reset_sn_list(sns.clone());
        }

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

        task::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn device_online_once(sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>) -> Device {
    let de = device_stack_new(1, sns.clone(), endpoints.clone(), 1, 0).await;
    let stack = {
        let stacks = de.0.stacks.lock().unwrap();
        let stack = stacks.get(0).unwrap();
        stack.clone()
    };

    let device = {
        let devices = de.0.devices.read().unwrap();
        let d = devices.get(0).unwrap();
        d.clone()
    };

    stack_sn_online(stack.clone(), sns.clone(), 3).await;
    let endpoint: &Endpoint = endpoints.get(0).unwrap();
    let ep = new_endpoint(&endpoint, 2);
    let mut eps = vec![];
    eps.push(ep);
    let _ = stack.reset_endpoints(&eps).await;

    device
}

//port:0,1,2
async fn remote_offline(
    sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>,
    call_interval_ms: u64,
    result: SnBenchResult,
    bench_end_time: u64) {
    let de = device_stack_new(1, sns.clone(), endpoints.clone(), 0, 0).await;
    let local_stack = {
        let stacks = de.0.stacks.lock().unwrap();
        let stack = stacks.get(0).unwrap();
        stack.clone()
    };

    stack_sn_online(local_stack.clone(), sns.clone(), 3).await;
    let remote = device_online_once(sns.clone(), endpoints).await;

    let pinger = cyfs_bdt::debug::Pinger::open(local_stack.clone().to_weak()).unwrap();
    let mut end_time = bucky_time_now() + 20;
    if end_time > bench_end_time {
        end_time = bench_end_time;
    }
    loop {
        if bucky_time_now() >= end_time {
            break ;
        }

        match pinger.ping(remote.clone(), Duration::from_secs(3), "debug".as_ref()).await {
            Ok(_) => {
            },
            Err(e) => {
                result.add_error(ExceptionType::RemoteOffline, e.code())
            }
        }
    
        task::sleep(Duration::from_millis(call_interval_ms)).await;
    }
}

async fn exception_remote_offline(
    sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>,
    call_interval_ms: u64,
    result: SnBenchResult,
    bench_time_sec: u64) {
    let bench_end_time = bucky_time_now() + bench_time_sec;
    loop {
        if bucky_time_now() >= bench_end_time {
            break ;
        }

        remote_offline(sns.clone(), endpoints.clone(), call_interval_ms, result.clone(), bench_end_time).await;
    }
}

//port:3,4
async fn exception_remote_not_exist(
    sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>,
    call_interval_ms: u64,
    result: SnBenchResult,
    bench_time_sec: u64) {
    let de = device_stack_new(2, sns.clone(), endpoints, 3, 0).await;
    let local_stack = {
        let stacks = de.0.stacks.lock().unwrap();
        let stack = stacks.get(0).unwrap();
        stack.clone()
    };
    let remote = {
        let devices = de.0.devices.read().unwrap();
        let d = devices.get(1).unwrap();
        d.clone()
    };

    stack_sn_online(local_stack.clone(), sns.clone(), 3).await;

    let pinger = cyfs_bdt::debug::Pinger::open(local_stack.clone().to_weak()).unwrap();
    let bench_end_time = bucky_time_now() + bench_time_sec;
    loop {
        if bucky_time_now() >= bench_end_time {
            break ;
        }

        match pinger.ping(remote.clone(), Duration::from_secs(3), "debug".as_ref()).await {
            Ok(_) => {
            },
            Err(e) => {
                result.add_error(ExceptionType::RemoteNotExists, e.code())
            }
        }

        task::sleep(Duration::from_millis(call_interval_ms)).await;
    }
}

pub async fn sn_bench_call(
    device_num: u64, device_dir: &str,
    sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>, 
    bench_time_sec: u64,
    call_interval_ms: u64,
    timeout_sec: u64,
    exception: bool) -> BuckyResult<SnBenchResult> {
    let call_bench_result = SnBenchResult::default();

    //
    if exception {
        println!("start exception task");

        let mut tasks = vec![];

        let start = bucky_time_now();
        //remote offline
        let snsl = sns.clone();
        let ep = endpoints.clone();
        let result: SnBenchResult = call_bench_result.clone();
        tasks.push(task::spawn(async move {
            exception_remote_offline(snsl, ep, call_interval_ms, result, bench_time_sec).await;
        }));

        //remote not exist
        let snsl = sns.clone();
        let ep = endpoints.clone();
        let result: SnBenchResult = call_bench_result.clone();
        tasks.push(task::spawn(async move {
            exception_remote_not_exist(snsl, ep, call_interval_ms, result, bench_time_sec).await;
        }));

        for t in tasks {
            let _ = t.await;
        }
        let end = bucky_time_now();

        call_bench_result.stat(start, end);

        return Ok(call_bench_result)
    }

    println!("init devices");
    let device_emulator = {
        if device_dir.len() != 0 { //load the dir's desc files
            unimplemented!()
        } else { //create device
            let de = device_stack_new(device_num, sns.clone(), endpoints, 0, 3600*24*30).await;
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
