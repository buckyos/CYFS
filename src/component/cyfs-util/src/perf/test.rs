use crate::*;

use cyfs_base::*;

#[macro_use]
use super::helper::*;
use super::holder::*;
use super::trace::*;


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
        println!("begin run...");
        loop {
            let id = perf_request_unique_id();
            perf_begin_request!(self.perf, "begin1", &id);

            perf_scope_request!(self.perf, "run");

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;

            perf_scope_request!(self.perf, "run2");

            perf_acc!(self.perf, "acc1");
            perf_acc!(self.perf, "acc2", BuckyErrorCode::InvalidData);
            perf_acc!(self.perf, "acc3", BuckyErrorCode::Ok, 1000);

            perf_record!(self.perf, "record1", 100);
            perf_record!(self.perf, "record1", 100, 1024);

            // perf_begin_request!()
            async_std::task::sleep(std::time::Duration::from_secs(5)).await;

            perf_end_request!(self.perf, "begin1", &id);
        }
    }
}

async fn test() {
    let first = First::new();

    async_std::task::sleep(std::time::Duration::from_secs(10)).await;

    let perf = TracePerf::new("root");
    first.perf.bind(Box::new(perf));

    async_std::task::sleep(std::time::Duration::from_secs(60 * 5)).await;
}

#[test]
fn main() {
    crate::init_log("test-perf", Some("trace"));

    async_std::task::block_on(async move {
        test().await;
    });
}
