use super::access::*;
use super::config::*;
use super::link::*;
use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStatePathMeta {
    access: GlobalStatePathAccessList,
    link: GlobalStatePathLinkList,
    config: GlobalStatePathConfigList,
}

impl Default for GlobalStatePathMeta {
    fn default() -> Self {
        Self {
            access: GlobalStatePathAccessList::default(),
            link: GlobalStatePathLinkList::default(),
            config: GlobalStatePathConfigList::default(),
        }
    }
}

declare_collection_codec_for_serde!(GlobalStatePathMeta);

pub struct GlobalStatePathMetaManager {
    dec_id: Option<ObjectId>,
    data: cyfs_lib::NOCCollectionRWSync<GlobalStatePathMeta>,
}

const CYFS_GLOBAL_STATE_PATH_META: &str = ".cyfs/meta";

impl GlobalStatePathMetaManager {
    pub async fn load(
        global_state: GlobalStateOutputProcessorRef,
        dec_id: Option<ObjectId>,
        noc: Box<dyn NamedObjectCache>,
    ) -> BuckyResult<Self> {
        let data = cyfs_lib::NOCCollectionRWSync::<GlobalStatePathMeta>::new_global_state(
            global_state,
            dec_id.clone(),
            CYFS_GLOBAL_STATE_PATH_META.to_owned(),
            None,
            "cyfs-global-state-path-meta",
            noc,
        );

        if let Err(e) = data.load().await {
            // FIXME 如果加载失败要如何处理，需要初始化为空还是直接返回错误终止执行？
            error!(
                "load global state path meta failed! dec={:?}, {}",
                dec_id, e
            );
            return Err(e);
        }

        Ok(Self { dec_id, data })
    }

    async fn select() {}
}
