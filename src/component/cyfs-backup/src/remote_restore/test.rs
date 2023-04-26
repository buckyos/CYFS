use cyfs_base::BuckyErrorCode;

use super::*;

use std::sync::Arc;

async fn download_folder() {
    let manager = Arc::new(RemoteRestoreManager::new());

    const ID: &str = "remote_store1";
    let params = RemoteRestoreParams {
        id: ID.to_owned(),

        cyfs_root: Some("C://cyfs/tmp/remote_store".to_owned()),
        isolate: None,
        password: None,

        remote_archive: "http://127.0.0.1:8887/test65/${filename}?token=123456".to_owned(),
    };

    let manager1 = manager.clone();
    async_std::task::spawn(async move {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(2)).await;

            let ret = manager1.get_task_status(ID);
            match ret {
                Ok(status) => {
                    println!("status: {}", serde_json::to_string(&status).unwrap());
                }
                Err(e) => {
                    if e.code() == BuckyErrorCode::NotFound {
                        warn!("restore task not found! {}", ID);
                        break;
                    } else {
                        unreachable!();
                    }
                }
            }
        }
    });

    // test cancel
    let manager1 = manager.clone();
    async_std::task::spawn(async move {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(20)).await;

            manager1.abort_task(ID).await.unwrap();
        }
    });

    match manager.run_remote_restore(params).await {
        Ok(()) => {
            info!("run restore task complete!");
        }
        Err(e) => {
            match e.code() {
                BuckyErrorCode::Aborted => {
                    warn!("restore task aborted!");
                }
                _ => {
                    unreachable!();
                }
            }
        }
    }
}

async fn download_file() {
    let manager = Arc::new(RemoteRestoreManager::new());

    const ID: &str = "remote_store1";
    let params = RemoteRestoreParams {
        id: ID.to_owned(),

        cyfs_root: Some("C://cyfs/tmp/remote_store".to_owned()),
        isolate: None,
        password: None,

        remote_archive: "http://127.0.0.1:8887/test65.zip?token=123456".to_owned(),
    };

    let manager1 = manager.clone();
    async_std::task::spawn(async move {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(2)).await;

            let status = manager1.get_task_status(ID).unwrap();
            println!("status: {}", serde_json::to_string(&status).unwrap());
        }
    });

    manager.run_remote_restore(params).await.unwrap();
}

#[test]
fn test() {
    cyfs_base::init_simple_log("test-remote-restore", None);
    async_std::task::block_on(download_folder());
}
