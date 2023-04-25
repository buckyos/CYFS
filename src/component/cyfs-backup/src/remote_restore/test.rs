
use super::*;
use crate::archive_download::*;

use std::sync::Arc;

async fn download_folder() {
    let manager = Arc::new(RemoteRestoreManager::new());

    let url = RemoteArchiveUrl {
        base_url: "http://127.0.0.1:8887/test65".to_owned(),
        file_name: None,
        query_string: Some("token=123456".to_owned()),
    };

    const ID: &str = "remote_store1";
    let params = RemoteRestoreParams {
        id: ID.to_owned(),

        cyfs_root: Some("C://cyfs/tmp/remote_store".to_owned()),
        isolate: None,
        password: None,

        remote_archive: RemoteArchiveInfo::Folder(url),
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

async fn download_file() {
    let manager = Arc::new(RemoteRestoreManager::new());

    let url = RemoteArchiveUrl {
        base_url: "http://127.0.0.1:8887/test65.zip".to_owned(),
        file_name: None,
        query_string: Some("token=123456".to_owned()),
    };

    const ID: &str = "remote_store1";
    let params = RemoteRestoreParams {
        id: ID.to_owned(),

        cyfs_root: Some("C://cyfs/tmp/remote_store".to_owned()),
        isolate: None,
        password: None,

        remote_archive: RemoteArchiveInfo::ZipFile(url),
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
    async_std::task::block_on(download_file());
}
