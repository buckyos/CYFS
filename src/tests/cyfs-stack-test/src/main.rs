#![recursion_limit = "256"]

mod case;
mod loader;

#[macro_use]
extern crate log;

use cyfs_debug::*;

async fn main_run() {
    CyfsLoggerBuilder::new_app("cyfs-stack-test")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("error"), Some("error"))
        .disable_file_config(true)
        .file(true)
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tests", "cyfs-stack-test")
        .exit_on_panic(true)
        .build()
        .start();

    loader::load().await;

    case::test().await;
    // info!("test process now will exits!");
}

fn main() {
    async_std::task::block_on(main_run())
}

#[cfg(test)]
mod main_tests {
    use super::*;
    use async_std::sync::Arc;
    use cyfs_base::*;
    use cyfs_core::*;
    use cyfs_lib::*;
    use std::str::FromStr;

    pub const DEC_ID: &'static str = "9tGpLNnSx4GVQTqg5uzUucPbK1TNJdZk3nNA77PPJaPW";

    #[async_std::test]
    async fn test_remove_panic() {
        let dec_id = ObjectId::from_str(DEC_ID).unwrap();
        let stack = Arc::new(SharedCyfsStack::open_default(Some(dec_id)).await.unwrap());
        stack.wait_online(None).await.unwrap();
        let env = stack
            .root_state_stub(None, None)
            .create_path_op_env()
            .await
            .unwrap();

        let header = "cyfs system";
        let value = "xxxxx";
        let obj = Text::create("cyfs", header, value);
        let r = stack
            .non_service()
            .put_object(NONPutObjectOutputRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    dec_id: None,
                    level: NONAPILevel::Router,
                    target: None,
                    flags: 0,
                },
                object: NONObjectInfo {
                    object_id: obj.desc().object_id().clone(),
                    object_raw: obj.to_vec().unwrap(),
                    object: None,
                },
            })
            .await
            .unwrap();
        let object_id = obj.desc().object_id();
        let ret = env
            .insert_with_key("/test/", "test_panic", &object_id)
            .await
            .unwrap();
        println!("insert {:?}", ret);
        let root = env.commit().await.unwrap();
        println!("new dec root is: {:?}", root);

        let env2 = stack
            .root_state_stub(None, None)
            .create_path_op_env()
            .await
            .unwrap();
        env2.remove_with_key("/test/", "test_panic", Some(object_id))
            .await
            .unwrap();
        // env2.remove_with_path("/test/panic",  Some(object_id)).await.unwrap();
        let root = env2.commit().await.unwrap();
    }
}
