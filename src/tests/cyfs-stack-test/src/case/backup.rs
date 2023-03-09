use cyfs_backup::*;
use cyfs_base::*;
use zone_simulator::*;

pub async fn test() {
    // 使用协议栈本身的dec_id
    let dec_id = TestLoader::get_dec_id();

    let user1_ood = TestLoader::get_stack(DeviceIndex::User1OOD);

    let params = UniBackupParams {
        id: bucky_time_now(),
        file: LocalFileBackupParam::default(),
    };

    user1_ood.backup_manager().run_uni_backup(params).await;
}
