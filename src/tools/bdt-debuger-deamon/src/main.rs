use std::{
    io::{Read}, 
    path::Path, 
    str::FromStr, 
    net::{Shutdown}, 
    time::Duration, 
    sync::{
        Arc,
        Mutex,
        RwLock,
    }
};
use async_std::{
    task,
    stream::StreamExt,
    io::{
        prelude::{
            ReadExt
        }, 
        WriteExt
    },
    future,
};
use clap::{
    App, 
    Arg
};
use cyfs_base::*;
use cyfs_bdt::*;
use log::*;
use cyfs_bdt::DatagramOptions;

fn load_dev_by_path(path: &str) -> Option<Device> {
    let desc_path = Path::new(path);
    if desc_path.exists() {
        let mut file = std::fs::File::open(desc_path).unwrap();
        let mut buf = Vec::<u8>::new();
        let _ = file.read_to_end(&mut buf).unwrap();
        let (device, _) = Device::raw_decode(buf.as_slice()).unwrap();
    
        Some(device)
    } else {
        None
    }
}

fn load_dev_vec(path: &str) -> Option<Vec<Device>> {
    let mut dev_vec = Vec::new();
    match load_dev_by_path(path) {
        Some(dev) => {
            dev_vec.push(dev);

            Some(dev_vec)
        },
        _ => None
    }
}

fn load_sn(sns: Vec<&str>) -> Option<Vec<Device>> {
    let mut dev_vec = Vec::new();

    if sns.len() == 0 {
        match load_dev_by_path("sn-miner.desc") {
            Some(dev) => {
                dev_vec.push(dev);
                Some(dev_vec)
            },
            _ => None
        }
    } else {
        for sn in sns {
            let dev = load_dev_by_path(sn).unwrap();
            dev_vec.push(dev);
        }

        Some(dev_vec)
    }
}
fn create_device(sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>) -> (Device, PrivateKey) {
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

fn loger_init(quiet: u16, name: &str) {
    if quiet == 0 {
        cyfs_debug::CyfsLoggerBuilder::new_app(name)
            .level("info")
            .console("info")
            .build()
            .unwrap()
            .start();

        cyfs_debug::PanicBuilder::new(name, name)
        .exit_on_panic(true)
        .build()
        .start();
    }

}

#[async_std::main]
async fn main() {
    let matches = App::new("bdt-debuger-deamon").version("1.0.0").about("deamon of bdt stack for debuger")
                        // .arg(Arg::with_name("desc").help("desc file of local device"))
                        // .arg(Arg::with_name("secret").help("private key file of local device"))
                        // .arg(Arg::with_name("sn").help("sn desc file to ping"))
                        .arg(Arg::with_name("listen").long("listen").takes_value(true).help("listen stream on vport"))
                        .arg(Arg::with_name("local").long("local").short("l").takes_value(true).default_value("127.0.0.1").help("debug command local ip to listen"))
                        .arg(Arg::with_name("port").long("port").short("p").takes_value(true).default_value("12345").help("debug command port to listen"))
                        .arg(Arg::with_name("ep").long("ep").multiple(true).takes_value(true).help("local endpoint"))
                        .arg(Arg::with_name("sn_conn_timeout").takes_value(true).default_value("0").help("sn connect timeout"))
                        .arg(Arg::with_name("active_pn").long("active_pn").takes_value(true).default_value("").help("active pn"))
                        .arg(Arg::with_name("passive_pn").long("passive_pn").takes_value(true).default_value("").help("passive pn"))
                        .arg(Arg::with_name("device_cache").long("device_cache").takes_value(true).default_value("").help("device cache"))
                        .arg(Arg::with_name("quiet").long("quiet").takes_value(false).default_value("0").help("quiet mode"))
                        .arg(Arg::with_name("udp_sn_only").long("udp_sn_only").takes_value(false).default_value("0").help("udp sn only"))
                        .arg(Arg::with_name("sn_bench").long("sn_bench").takes_value(false).default_value("0").help("sn bench, clients"))
                        .arg(Arg::with_name("sn").long("sn").multiple(true).takes_value(false).default_value("").help("sn desc file"))
                        .get_matches();

    let mut endpoints = vec![];
    for ep in matches.values_of("ep").unwrap() {
        if let Ok(ep) = Endpoint::from_str(ep) {
            endpoints.push(ep);
        } else {
            println!("invalid endpoint {}", ep);
            return;
        }
    }

    let quiet = u16::from_str(matches.value_of("quiet").unwrap()).unwrap();
    let udp_sn_only = u16::from_str(matches.value_of("udp_sn_only").unwrap()).unwrap();
    let sn_bench = usize::from_str(matches.value_of("sn_bench").unwrap()).unwrap();

    let mut sns = vec![];
    for sn in matches.values_of("sn").unwrap() {
        if sn.len() != 0 {
            sns.push(sn);
        }
    }
    let sns = load_sn(sns);

    if sn_bench > 0 {
        loger_init(quiet, "sn_bench");
        sn_bench_do(sn_bench, sns.clone(), endpoints.clone()).await;
        return ;
    }

    let default_desc_path = Path::new("deamon.desc");
    let default_sec_path = Path::new("deamon.sec");
    if !default_desc_path.exists() || !default_sec_path.exists() {
        println!("deamon.desc not exists, generate new one");

        let (device, private_key) = create_device(sns.clone(), endpoints.clone());

        if let Err(err) = device.encode_to_file(&default_desc_path, false) {
            println!("generate deamon.desc failed for {}",  err);
            return;
        } 
        if let Err(err) = private_key.encode_to_file(&default_sec_path, false) {
            println!("generate deamon.sec failed for {}",  err);
            return;
        }
    }
    let desc_path = matches.value_of("desc").map(|s| Path::new(s)).unwrap_or(default_desc_path);
    let sec_path = matches.value_of("secret").map(|s| Path::new(s)).unwrap_or(default_sec_path);
    if !desc_path.exists() {
        println!("{:?} not exists", desc_path);
        return;
    }
    let device = {
        let mut buf = vec![];
        Device::decode_from_file(&desc_path, &mut buf).map(|(d, _)| d)
    }; 

    if let Err(err) = device {
        println!("load {:?} failed for {}", desc_path, err);
        return;
    } 
    let mut device = device.unwrap();
    info!("device={:?}", device);

    let private_key = {
        let mut buf = vec![];
        PrivateKey::decode_from_file(&sec_path, &mut buf).map(|(k, _)| k)
    }; 
    
    if let Err(err) = private_key {
        println!("load {:?} failed for {}", sec_path, err);
        return;
    } 
    let private_key = private_key.unwrap();

    let deamon_id = device.desc().device_id();
    let deamon_name = format!("bdt-debuger-deamon-{}", deamon_id);
    loger_init(quiet, deamon_name.as_str());

    let device_endpoints = device.mut_connect_info().mut_endpoints();
    device_endpoints.clear();
    for ep in endpoints {
        device_endpoints.push(ep);
    }

    let local = matches.value_of("local").unwrap();
    let port = u16::from_str(matches.value_of("port").unwrap());
    if port.is_err() {
        println!("invalid arg port");
        return;
    }
    let port = port.unwrap();

    let chunk_store = MemChunkStore::new();
    let mut params = StackOpenParams::new(deamon_name.as_str());
    params.config.debug = Some(cyfs_bdt::debug::Config {
        local: local.to_string(),
        port,
        chunk_store: chunk_store.clone(),
    });
    let sns2 = sns.clone();
    params.known_sn = sns;
    if udp_sn_only != 0 {
        params.config.interface.udp.sn_only = true;
        println!("sn_only is set");
    } else {
        params.config.interface.udp.sn_only = false;
    }

    if let Some(active_pn) = matches.value_of("active_pn") {
        params.active_pn = load_dev_vec(active_pn);
    }
    if let Some(passive_pn) = matches.value_of("passive_pn") {
        params.passive_pn = load_dev_vec(passive_pn);
    }
    params.chunk_store = Some(Box::new(chunk_store.clone()));

    let stack = Stack::open(
        device, 
        private_key, 
        params).await;
        
    if let Err(err) = stack {
        println!("open stack failed for {}", err);
        return ;
    }

    let stack = stack.unwrap();

    if sns2.is_some() {
        stack.reset_sn_list(sns2.unwrap());
    }

    match future::timeout(
        Duration::from_secs(5),
        stack.sn_client().ping().wait_online(),
    ).await {
        Ok(res) => {
            match res {
                Ok(res) => {
                    match res {
                        SnStatus::Online => {},
                        _ => {
                            println!("sn offline");
                        }
                    }
                },
                Err(e) => {
                    println!("connect sn err={}", e);
                }
            }
        },
        Err(e) => {
            println!("wait_online err={}", e);
        }
    }


    if let Some(device_cache) = matches.value_of("device_cache") {
        if device_cache.len() > 0 {
            let dev = load_dev_by_path(device_cache).unwrap();
            let device_id = dev.desc().device_id();
            stack.device_cache().add(&device_id, &dev);
        }
    }

    if let Some(vport) = matches.value_of("listen") {
        let vport = u16::from_str(vport);
        if vport.is_err() {
            println!("invalid arg listen");
            return;
        }
        let vport = vport.unwrap();
        let listener = stack.stream_manager().listen(vport);        
        if let Err(err) = listener {
            println!("listen stream on port {} failed for {}", vport, err);
            return;
        }
        println!("listen stream on port {}", vport);
        let listener = listener.unwrap();
        task::spawn(async move {
            let mut incoming = listener.incoming();
            loop {
                if let Some(stream) = incoming.next().await {
                    if let Ok(mut stream) = stream {
                        task::spawn(async move {
                            println!("question len={} content={:?}", 
                                stream.question.len(), String::from_utf8(stream.question).expect(""));

                            let answer = b"answer!";
                            let _ = stream.stream.confirm(&answer.to_vec()).await;

                            let mut read_buf = [0; 128];
                            if let Ok(n) = stream.stream.read(&mut read_buf).await {
                                println!("read len={} data={}", n, String::from_utf8(read_buf[..n].to_vec()).expect(""));
                            }

                            let write_buf = b"abcdefg";
                            if let Ok(n) = stream.stream.write(write_buf).await {
                                println!("write len={}", n);
                            }

                            task::spawn(async move {
                                let mut buf = vec![0u8; 2048];
                                let mut total = 0;
                                loop {
                                    match stream.stream.read(&mut buf).await {
                                        Ok(n) => {
                                            total += n;
                                            if n == 0 {
                                                break;
                                            }
                                        },
                                        Err(e) => {
                                            println!("read err={}", e);
                                            break;
                                        }
                                    }
                                }
                                task::sleep(std::time::Duration::from_millis(200)).await;
                                match stream.stream.shutdown(Shutdown::Both) {
                                    Ok(_) => println!("shutdown ok, total={}", total),
                                    Err(e) => println!("shutdown err: {:?}", e),
                                }
                            });
                        });
                    }
                }
            }
        });
    }

    println!("stack debug deamon running...");

    async_std::future::pending::<()>().await;
}

struct BencData {
    start: u64,
    end: u64,
    err: String,
    online_try: usize,

    ping_try: usize,
    ping_success: usize,
    ping_resptime_min: usize,
    ping_resptime_max: usize,
    ping_resptime: usize,

    call_try: usize,
    call_success: usize,
    call_time_min: u64,
    call_time_max: u64,
    call_time: u64,
}

struct SnBencherImpl {
    stack: StackGuard,
    data: Mutex<BencData>,
    index: usize,
}

struct SnBencher(Arc<SnBencherImpl>);

fn sn_bench_new(stack: StackGuard, index: usize) -> SnBencher {
    SnBencher(Arc::new(SnBencherImpl{
        stack,
        data: Mutex::new(BencData {
            start: 0,
            end: 0,
            err: format!(""),
            online_try: 0,
            ping_try: 0,
            ping_success: 0,
            ping_resptime_min: 0,
            ping_resptime_max: 0,
            ping_resptime: 0,
            call_try: 0,
            call_success: 0,
            call_time_min: 0,
            call_time_max: 0,
            call_time: 0,
        }),
        index,
    }))
}

fn new_endpoint(ep: &Endpoint, index: u16) -> Endpoint {
    let mut endpoint = Endpoint::default();
    endpoint.clone_from(ep);

    let addr = endpoint.mut_addr();
    let port = ep.addr().port()+index;
    addr.set_port(port);

    endpoint
}

async fn sn_bench_do(clients: usize, sns: Option<Vec<Device>>, endpoints: Vec<Endpoint>) {
    const SN_ONLINE_TIMEOUT_SEC: u64 = 10;
    const SN_ONLINE_TRY_MAX: usize = 3;
    const CREATE_DEVICE_THREADS: i32 = 16;
    const VPORT: usize = 9999;
    const PING_TIMEOUT_SEC: usize = 5;
    //
    const PING_NUM_MAX: usize = 1;
    const CALL_NUM_MAX: usize = 500;

    let endpoint = endpoints.get(0).unwrap();
    let name = format!("{}", endpoint.addr().to_string());

    /*
    create device
    */
    println!("create devices.");
    let mut sn_benchers = vec![];
    let stacks = Arc::new(Mutex::new(vec![]));
    let stacks_counter = Arc::new(Mutex::new(0));
    let mut tasks = vec![];
    let devices = Arc::new(RwLock::new(vec![]));
    for i in 0..CREATE_DEVICE_THREADS {
        let s = stacks.clone();
        let sc = stacks_counter.clone();
        let ept = endpoint.clone();
        let snsl = sns.clone();
        let devs = devices.clone();
        tasks.push(task::spawn(async move {
            loop {
                let idx = {
                    let mut st = sc.lock().unwrap();
                    if *st >= clients as i32 {
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
    {
        let stacks = stacks.lock().unwrap();
        for i in 0..stacks.len() {
            let stack = stacks.get(i).unwrap();
            sn_benchers.push(sn_bench_new(stack.clone(), i));
        }
    }
    println!("create finish.");

    //get_bench_flag(name.clone()).await.unwrap();

    /*
    sn ping
    */
    println!("start bench..");
    let mut tasks = vec![];
    let begin = bucky_time_now();
    for snb in &sn_benchers {
        let bencher = snb.0.clone();
        let snsl = sns.clone();
        tasks.push(task::spawn(async move {
            let mut num = 0;
            loop {
                let mut online = false;

                let ts = bucky_time_now();
                match future::timeout(
                    Duration::from_secs(SN_ONLINE_TIMEOUT_SEC),
                    bencher.stack.sn_client().ping().wait_online(),
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
                let cost = ((bucky_time_now() - ts) / 1000) as usize;

                {
                    let mut data = bencher.data.lock().unwrap();
                    data.ping_try += 1;
                    if online {
                        data.ping_success += 1;
                        data.ping_resptime += cost;
                        if cost > 0 && (data.ping_resptime_min > cost || data.ping_resptime_min == 0) {
                            data.ping_resptime_min = cost;
                        }
                        if cost > data.ping_resptime_max {
                            data.ping_resptime_max = cost;
                        }
                    }
                    num += 1;
                }

                if num >= PING_NUM_MAX {
                    break;
                }
    
                if let Some(sns) = snsl.as_ref() {
                    bencher.stack.reset_sn_list(sns.clone());
                }
            }
        }));
    }

    for t in tasks {
        let _ = t.await;
    }
    let end = bucky_time_now();

    let mut stat_fail = 0;
    let mut stat_success = 0;
    let mut stat_online_time = 0;
    let mut stat_online_time_max = 0;
    let mut stat_online_time_min = 999999;
    for snb in &sn_benchers {
        let data = snb.0.data.lock().unwrap();

        stat_success += data.ping_success;
        stat_fail += data.ping_try - data.ping_success;

        stat_online_time += data.ping_resptime;
        if stat_online_time_max < data.ping_resptime_max {
            stat_online_time_max = data.ping_resptime_max;
        }
        if data.ping_resptime_min > 0 && (stat_online_time_min > data.ping_resptime_min) {
            stat_online_time_min = data.ping_resptime_min;
        }

        let debug_tunnel = snb.0.stack.datagram_manager().bind(VPORT as u16).unwrap();
        task::spawn(async move {
            loop {
                match debug_tunnel.recv_v().await {
                    Ok(datagrams) => {
                        for datagram in datagrams {
                            let mut options = datagram.options.clone();
                            let _ = debug_tunnel.send_to(
                                datagram.data.as_ref(),
                                &mut options, 
                                &datagram.source.remote, 
                                datagram.source.vport);
                        }
                    }, 
                    Err(_err) => {
                    }
                }
            }
        });
    }

    let stat_online_time_ave = if stat_success > 0 {
        stat_online_time / stat_success
    } else {
        0
    };
    let qps = if stat_online_time_ave > 0 {
        let x = stat_online_time / 1000;
        if x > 0 {
            stat_success / x
        } else {
            0
        }
    } else {
        0
    };

    let total_cost = ((end-begin)/1000) as usize;
    let qps2 = {
        let x = total_cost/1000;
        if x > 0 {
            stat_success/x
        } else {
            0
        }
    };

    println!("\ntime_elsp={} ms", total_cost);
    println!("success={}\nfail={}\nonline_time={} ms\nonline_time_max={} ms\nonline_time_min={} ms\nqps={}\ntotal_cost={} s\nqps2={}\n",
        stat_success, stat_fail, stat_online_time_ave, stat_online_time_max, stat_online_time_min, 
        qps, total_cost/1000, qps2);

    /*
    sn call
    */
    get_bench_flag(name.clone()).await.unwrap();

    let mut tasks = vec![];
    let mut call_count = clients / 2;
    let offset = call_count;
    let start = bucky_time_now();
    for snb in &sn_benchers {
        call_count -= 1;
        if call_count <= 0 {
            break ;
        }
        let bencher = snb.0.clone();
        let devs = devices.clone();
        tasks.push(task::spawn(async move {
            let remote = {
                let d = devs.read().unwrap();
                let mut idx = bencher.index+offset;
                if idx >= clients {
                    idx = 0;
                }
                let d = d.get(idx).unwrap().clone();

                let device_id = d.desc().device_id();
                bencher.stack.device_cache().add(&device_id, &d);

                d
            };

            let datagram = bencher.stack.datagram_manager().bind((VPORT+1) as u16).unwrap();
            let mut num = 0;
            loop {
                if num >= CALL_NUM_MAX {
                    break ;
                }

                bencher.stack.tunnel_manager().reset();

                let mut options = DatagramOptions::default();
                let ts = bucky_time_now();
                options.sequence = Some(TempSeq::from(ts as u32));
                let _ = datagram.send_to(
                    "debug".as_ref(), 
                    &mut options, 
                    &remote.desc().device_id(), 
                    VPORT as u16);
                match future::timeout(Duration::from_secs(PING_TIMEOUT_SEC as u64), datagram.recv_v()).await {
                    Err(_) => {
                    },
                    Ok(res) => {
                        let datagrams = res.unwrap();
                        for datagram in datagrams {
                            if let Some(opt) = datagram.options.sequence {
                                if opt == options.sequence.unwrap() {
                                    let cost = (bucky_time_now() - ts) / 1000;
                                    let mut data = bencher.data.lock().unwrap();
                                    data.call_success += 1;
                                    data.call_time += cost;
                                    break ;
                                }
                            }
                        }
                    }
                }

                {
                    let mut data = bencher.data.lock().unwrap();
                    data.call_try += 1;
                    num += 1;
                }
            }
        }));
    }
    for t in tasks {
        let _ = t.await;
    }
    let end = bucky_time_now();
    let cost = ((end - start) / 1000) as usize;

    let mut stat_fail = 0;
    let mut stat_retry_client = 0;
    let mut stat_success = 0;
    let mut stat_call_time = 0;
    let mut stat_call_time_max = 0;
    let mut stat_call_time_min = 0;
    let mut retry_client = 0;
    for snb in &sn_benchers {
        let data = snb.0.data.lock().unwrap();
        stat_success += data.call_success;
        stat_fail += data.call_try - data.call_success;
        stat_retry_client += data.call_try;

        let t = data.call_time;
        stat_call_time += t;
        if t > stat_call_time_max {
            stat_call_time_max = t;
        }
        if t > 0 && (t < stat_call_time_min || stat_call_time_min == 0) {
            stat_call_time_min = t;
        }
        if data.call_try > 1 {
            retry_client += 1;
        }
    }
    let stat_call_time_ave = if stat_success > 0 {
        stat_call_time / 1000 / (stat_success as u64)
    } else {
        0
    };
    let qps = {
        let x = cost / 1000;
        if x > 0 {
            stat_success / x
        } else {
            0
        }
    };
    println!("\nsuccess={}\nfail={}\nnc_time={} ms\nnc_time_max={} ms\nnc_time_min={} ms\nretry_client={}\nqps={}\ntry={}\n",
        stat_success, stat_fail, stat_call_time_ave, stat_call_time_max/1000, stat_call_time_min/1000,
        retry_client, qps, stat_retry_client);
}

async fn get_bench_flag(_name: String) -> BuckyResult<()> {
    println!("press enter to next action");
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).expect("failed to read line");

    Ok(())
}
