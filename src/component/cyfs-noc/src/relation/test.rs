use crate::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

fn new_object(id: usize) -> NONObjectInfo {
    let obj = Text::create(&id.to_string(), "", "");
    NONObjectInfo::new_from_object_raw(obj.to_vec().unwrap()).unwrap()
}

async fn test_relation() {
    cyfs_base::init_simple_log("cyfs-noc-relation-test", Some("debug"));

    let noc = NamedObjectRelationCacheManager::create("test")
        .await
        .unwrap();

    let object = new_object(0);
    let cache_key = NamedObjectRelationCacheKey {
        object_id: object.object_id.clone(),
        relation_type: NamedObjectRelationType::InnerPath,
        relation: "/a/b".to_owned(),
    };

    for i in 1..100 {
        let target_object = new_object(i);
        let req = NamedObjectRelationCachePutRequest {
            cache_key: cache_key.clone(),
            target_object_id: target_object.object_id.clone(),
        };

        noc.put(&req).await.unwrap();

        let mut get_req = NamedObjectRelationCacheGetRequest {
            cache_key: cache_key.clone(),
            flags: 0,
        };

        let data = noc.get(&get_req).await.unwrap().unwrap();
        assert_eq!(data.target_object_id, req.target_object_id);

        // missing get
        let target_object = new_object(i * 1000);
        get_req.cache_key.object_id = target_object.object_id.clone();

        let ret = noc.get(&get_req).await.unwrap();
        assert!(ret.is_none());
    }
}

#[test]
fn main() {
    async_std::task::block_on(async move {
        test_relation().await;
    });
}
