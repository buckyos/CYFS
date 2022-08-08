use crate::*;

use cyfs_base::*;
use std::sync::Arc;

use super::helper::*;
use super::trace::*;

struct First {
    perf: PerfHolder,
    test1: Second,
    test2: Second,
}

impl First {
    pub fn new() -> Self {
        let perf = PerfHolder::new_isolate("first");
        let test1 = Second::new();
        test1.perf().bind(&perf);
        test1.start();

        let test2 = Second::new_empty();
        test2.perf().bind(&perf);
        test2.start();

        Self { perf, test1, test2 }
    }
}

#[derive(Clone)]
struct Second {
    perf: PerfHolder,
}

impl Second {
    pub fn new() -> Self {
        Self {
            perf: PerfHolder::new_isolate("second"),
        }
    }

    pub fn new_empty() -> Self {
        Self {
            perf: PerfHolder::new(),
        }
    }

    pub fn perf(&self) -> &PerfHolder {
        &self.perf
    }

    fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            this.run().await;
        });
    }

    async fn run(&self) {
        println!("begin run second...");

        
        loop {
            let id = perf_request_unique_id();
            perf_begin_request!(self.perf, "begin1", &id);

            perf_scope_request!(self.perf, "run");

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;

            perf_scope_request!(self.perf, "run2");

            perf_rev_scope_request!(self.perf, "run2", "test_case1");
            perf_rev_scope_request!(self.perf, "run2", "test_case2", BuckyErrorCode::InvalidData);

            perf_acc!(self.perf, "acc1");
            perf_acc!(self.perf, "acc2", BuckyErrorCode::InvalidData);
            perf_acc!(self.perf, "acc3", BuckyErrorCode::Ok, 1000);

            perf_record!(self.perf, "record1", 100);
            perf_record!(self.perf, "record1", 100, 1024);

            perf_action!(
                self.perf,
                "action1",
                BuckyErrorCode::AddrInUse,
                "key".to_owned(),
                "value".to_owned()
            );

            // perf_begin_request!()
            async_std::task::sleep(std::time::Duration::from_secs(5)).await;

            perf_end_request!(self.perf, "begin1", &id);

            let _ = self.test_scope(0).await;
            // perf_end_scope_request!(__perf, BuckyErrorCode::AddrInUse, None);
        }
    }

    async fn test_scope(&self, param: u32) -> BuckyResult<()> {
        perf_scope!(self.perf, "test_scope", {
            async_std::task::sleep(std::time::Duration::from_secs(5)).await;
            if param % 2 == 0 {
                println!("test scope 1......");
                async_std::task::sleep(std::time::Duration::from_secs(1)).await;
                Ok(())
            } else {
                println!("test scope 2......");
                Err(BuckyError::from(BuckyErrorCode::InvalidData))
            }
        })
    }
}

async fn test() {
    let first = First::new();

    async_std::task::sleep(std::time::Duration::from_secs(10)).await;

    let perf = TracePerf::new("root");
    first.perf.bind_raw(&Arc::new(Box::new(perf)));

    async_std::task::sleep(std::time::Duration::from_secs(60 * 5)).await;
}

#[test]
fn main() {
    crate::init_log("test-perf", Some("trace"));

    async_std::task::block_on(async move {
        test().await;
    });
}
