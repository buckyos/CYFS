use cyfs_base::ObjectMapContentItem;
use cyfs_core::{Text, TextObj};

use crate::sim_zone::SimZone;
use crate::bench::*;

pub struct Stat {}

impl Stat {

    pub async fn write(zone: &SimZone, key: &str, costs: u64) {
        // 汇总本地数据 rate, min/avg/max
        {
            let stack = zone.get_shared_stack("zone1_ood");
            let local_cache = stack.local_cache_stub(None);
            let local_op_env = local_cache.create_path_op_env().await.unwrap();
    
            let path = format!("/stat-{}/{}", key, costs.to_string());
            
            let object = Text::build(key, "header", costs.to_string())
            .no_create_time()
            .build();
            let object_id = object.text_id().object_id().to_owned();
    
            local_op_env.insert(path, &object_id).await.unwrap();
    
            let root = local_op_env.commit().await.unwrap();
            debug!("new local cache dec root is: {:?}", root);
        }
    }

    pub async fn read(zone: &SimZone, samples: u64) {
        let stack = zone.get_shared_stack("zone1_ood");
        stack.wait_online(None).await.unwrap();
        let local_cache = stack.local_cache_stub(None);
        let local_op_env = local_cache.create_path_op_env().await.unwrap();

        let arr = vec![NON_PUT_OBJECT, CROSS_LOCAL_STATE];
        for key in arr.into_iter() {
            let mut data = Vec::new();
            let path = format!("/stat-{}", key);
            let list = local_op_env.list(&path).await.unwrap();
            for item in list.into_iter() {
                match item {
                    ObjectMapContentItem::Map((key, _value)) => {
                        data.push(key.parse::<u32>().unwrap());
                    }
                    ObjectMapContentItem::Set(_value) => {
                    }
                    _ => unreachable!(),
                }
            }

            data.sort();
            let sum: u32 = data.iter().sum();
            info!("-----------------------------------------------------------------------");
            info!("test: {}, samples: {}, avg: {}ms, min: {}ms, max: {}ms", key, samples, sum / data.len() as u32, data[0],  data[data.len() - 1]);
            info!("-----------------------------------------------------------------------");

        }
    }

    pub async fn clear(zone: &SimZone) {
        let stack = zone.get_shared_stack("zone1_ood");
        stack.wait_online(None).await.unwrap();

        let arr = vec![NON_PUT_OBJECT, CROSS_LOCAL_STATE];
        for key in arr.into_iter() {
            let local_cache = stack.local_cache_stub(None);
            let local_op_env = local_cache.create_path_op_env().await.unwrap();
            let path = format!("/stat-{}", key);
            local_op_env.remove_with_path(path, None).await.unwrap();
    
            let root = local_op_env.commit().await.unwrap();
            debug!("new local cache dec root is: {:?}", root);
        }
    }
}