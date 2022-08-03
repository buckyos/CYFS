use criterion::{criterion_group, criterion_main, Criterion};
use criterion::async_executor::FuturesExecutor;

use cyfs_base::*;
use cyfs_core::*;

#[macro_use]
extern crate log;

use cyfs_util::*;

use cyfs_lib::*;
use cyfs_perf_client::{PerfIsolate, PerfServerConfig};

fn new_dec(name: &str) -> ObjectId {
    let owner_id = cyfs_base::ObjectId::default();

    let dec_id = DecApp::generate_id(owner_id.to_owned(), name);

    info!("generage test perf  dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

struct First {
    perf: PerfHolderRef,
    test1: Second,
    test2: Second,
}

impl First {
    pub fn new() -> Self {
        let perf = PerfHolder::new("first");
        let test1 = perf.fork("test1").unwrap();
        let test1 = Second::new(test1);
        test1.start();

        let test2 = perf.fork("test2").unwrap();
        let test2 = Second::new(test2);
        test2.start();

        Self { perf, test1, test2 }
    }
}

#[derive(Clone)]
struct Second {
    perf: PerfHolderRef,
}

impl Second {
    pub fn new(perf: PerfHolderRef) -> Self {
        Self { perf }
    }

    fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            this.run().await;
        });
    }
    async fn run(&self) {
        let dec_id = new_dec("test-perf");
        //let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
        let stack = SharedCyfsStack::open_default(Some(dec_id)).await.unwrap();
        stack.wait_online(None).await.unwrap();

        //let perf = TracePerf::new("root");, 实现了perf trait

        let perf = PerfIsolate::new(
            "root",
            60,
            dec_id,
            PerfServerConfig::Default,
            stack,
        );

        let dyn_perf = perf.fork("main").unwrap();

        self.perf.bind(dyn_perf);
        println!("begin run...");
        let id = perf_request_unique_id();
        perf_begin_request!(self.perf, "begin1", &id);
        perf_scope_request!(self.perf, "run");

        //async_std::task::sleep(std::time::Duration::from_secs(1)).await;

        perf_scope_request!(self.perf, "run2");

        perf_acc!(self.perf, "acc1");
        perf_acc!(self.perf, "acc2", BuckyErrorCode::InvalidData);
        perf_acc!(self.perf, "acc3", BuckyErrorCode::Ok, 1000);

        perf_record!(self.perf, "record1", 100);
        perf_record!(self.perf, "record1", 100, 1024);

        perf_action!(self.perf, "action1", BuckyErrorCode::InvalidData, "cyfs".to_owned(), "stack".to_owned());
        perf_action!(self.perf, "action2", BuckyErrorCode::Ok, "cyfs".to_string(), "bdt".to_string());

        // perf_begin_request!()
        println!("case run...");
        //async_std::task::sleep(std::time::Duration::from_secs(10)).await;

        perf_end_request!(self.perf, "begin1", &id, BuckyErrorCode::Ok, 100);

        perf.save_test().await;

    }
}

pub async fn test() {
    let first = First::new();

    info!("bind....");
}


fn perf(c: &mut Criterion) {

    // 进行 benchmark
    c.bench_function("test", move |b| {
        b.to_async(FuturesExecutor)
            .iter(|| async { test().await })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = perf
}
criterion_main!(benches);
