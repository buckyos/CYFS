use super::db::*;
use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

async fn test_meta() {
    let dir = cyfs_util::get_temp_path().join("test_noc_meta");
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).unwrap();
    }

    let meta = SqliteMetaStorage::new(&dir).unwrap();

    let req = NamedObjectMetaExistsObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: ObjectId::default(),
    };
    let ret = meta.exists_object(&req).await.unwrap();
    assert!(!ret);
}

#[test]
fn main() {
    cyfs_base::init_simple_log("cyfs-noc-test-meta", Some("debug"));

    async_std::task::block_on(async move {
        test_meta().await;
    });
}
