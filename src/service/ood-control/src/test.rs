use crate::*;
use cyfs_backup::*;


async fn start() {
    let param = ControlInterfaceParam {
        mode: OODControlMode::Runtime,
        tcp_port: None,

        // if device binded already，should not bind public address to avoid risk
        require_access_token: false,
        tcp_host: None,
        addr_type: ControlInterfaceAddrType::V4,
    };

    let control_interface = ControlInterface::new(param, &OOD_CONTROLLER);
    control_interface.start().await.unwrap();
}

async fn test_restore() {
    start().await;

    let control_url = "http://127.0.0.1:1321/restore";
    let param = RemoteRestoreParams::new("restore_task_1", "http://127.0.0.1:8887/test65/${filename}?token=123456");
    
    let resp = surf::post(control_url).body_json(&param).unwrap().await.unwrap();
    assert!(resp.status().is_success());

    let tasks_url = format!("{}/tasks", control_url);
    let mut resp = surf::get(&tasks_url).await.unwrap();
    assert!(resp.status().is_success());
    let list = resp.body_string().await.unwrap();
    info!("task list: {}", list);

    let mut count = 0;
    let task_url = format!("{}/{}", control_url, param.id);
    loop {
        let mut resp = surf::get(&task_url).await.unwrap();
        assert!(resp.status().is_success());

        let status = resp.body_string().await.unwrap();
        info!("status: {}", status);

        async_std::task::sleep(std::time::Duration::from_secs(5)).await;

        count += 1;
        if count >= 5 {
            break;
        }
    }

    let resp = surf::delete(&task_url).await.unwrap();
    assert!(resp.status().is_success());

    let resp = surf::get(&task_url).await.unwrap();
    assert_eq!(resp.status(), http_types::StatusCode::NotFound);
}

#[test]
fn test() {
    cyfs_base::init_simple_log("test-ood-control", None);
    async_std::task::block_on(test_restore());
}