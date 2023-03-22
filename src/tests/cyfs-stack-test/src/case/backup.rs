use cyfs_backup_lib::*;
use cyfs_backup::*;
use cyfs_base::*;
use cyfs_util::get_cyfs_root_path_ref;
use zone_simulator::*;

pub async fn test() {
    let user1_ood = TestLoader::get_stack(DeviceIndex::User1OOD);

    let isolate = match &user1_ood.config().get_stack_params().config.isolate {
        Some(isolate) => {
            isolate.to_owned()
        }
        None => {
            "".to_owned()
        }
    };

    let service = BackupService::new(&isolate).await.unwrap();

    let params = UniBackupParams {
        id: bucky_time_now().to_string(),
        isolate: isolate.clone(),
        target_file: LocalFileBackupParam::default(),
        password: Some(ProtectedPassword::new("123456")),
    };

    let target_dir = params.dir().to_path_buf();
    service.backup_manager().run_uni_backup(params).await.unwrap();

    let service = RestoreService::new(&isolate).await.unwrap();
    let params = UniRestoreParams {
        id: bucky_time_now().to_string(),
        // cyfs_root: get_cyfs_root_path_ref().as_os_str().to_string_lossy().to_string(),
        cyfs_root: get_cyfs_root_path_ref().join("tmp/restore").as_os_str().to_string_lossy().to_string(),
        isolate,
        archive: target_dir,
        password: Some(ProtectedPassword::new("123456")),
    };

    service.restore_manager().run_uni_restore(params).await.unwrap();
}
