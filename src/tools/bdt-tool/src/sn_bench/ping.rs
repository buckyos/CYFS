use std::{
    time::Duration, 
    sync::{
        Arc,
        RwLock,
    }
};
use async_std::{
    task,
    future,
};

use cyfs_base::*;
use cyfs_bdt::*;

use super::*;

pub async fn sn_bench_ping(
    device_num: u64, device_dir: &str,
    sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>, 
    bench_time_sec: u64,
    ping_interval_ms: u64,
    timeout_sec: u64,
    _: SnBenchPingException) -> BuckyResult<SnBenchResult> {
    let sn_bench_result = SnBenchResult::default();

    let device_emulator = {
        if device_dir.len() != 0 { //load the dir's desc files
            unimplemented!()
        } else { //create device
            let de = device_stack_new(device_num, sns.clone(), endpoints).await;
            device_save(de.clone()).await;
            de
        }
    };

    //benching
    let exit_flag = Arc::new(RwLock::new(false));
    let mut tasks = vec![];

    let start = bucky_time_now();
    for i in 0..device_num {
        let de = device_emulator.clone();
        let flag = exit_flag.clone();
        let snsl = sns.clone();
        let stacks = de.0.stacks.lock().unwrap();
        let stack = stacks.get(i as usize).unwrap().clone();
        let result = sn_bench_result.clone();
        tasks.push(task::spawn(async move {
            loop {
                {
                    if *flag.read().unwrap() {
                        break ;
                    }
                }

                let mut online = false;

                let ts = bucky_time_now();
                match future::timeout(
                    Duration::from_secs(timeout_sec),
                    stack.sn_client().ping().wait_online(),
                ).await {
                    Ok(res) => {
                        match res {
                            Ok(res) => {
                                match res {
                                    SnStatus::Online => {
                                        task::sleep(Duration::from_millis(ping_interval_ms)).await;

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
                let cost_ms = ((bucky_time_now() - ts) / 1000) as i16;

                {
                    if online {
                        result.add_resp_time(cost_ms);
                    } else {
                        result.add_resp_time(-1);
                    }
                }

                if let Some(sns) = snsl.as_ref() {
                    stack.reset_sn_list(sns.clone());
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

    //stat
    sn_bench_result.stat(start, end);

    Ok(sn_bench_result)
}
