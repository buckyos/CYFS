use cyfs_backup_lib::*;
use cyfs_backup::*;
use cyfs_base::*;
use zone_simulator::*;

pub async fn test() {
    // 使用协议栈本身的dec_id
    let dec_id = TestLoader::get_dec_id();

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
        isolate,
        target_file: LocalFileBackupParam::default(),
    };

    service.backup_manager().run_uni_backup(params).await;
}
