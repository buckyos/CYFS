use crate::*;
use cyfs_base::*;

use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NormalObjectPostion {
    Middle,
    Leaf,
    Assoc,
}

#[derive(Debug)]
pub struct NormalObject {
    pub pos: NormalObjectPostion,
    pub object: NONSlimObjectInfo,
    pub path: String,
    pub config_ref_depth: u32,
    pub ref_depth: u32,
}

impl NormalObject {
    pub fn dervie_path(path: &str, key: &str) -> String {
        if path.ends_with("/") {
            format!("{}{}", path, key)
        } else {
            format!("{}/{}", path, key)
        }
    }

    pub fn derive_normal(
        &self,
        object_id: ObjectId,
        key: Option<&str>,
        is_ref_object: bool,
    ) -> Self {
        let pos = match object_id.obj_type_code() {
            ObjectTypeCode::ObjectMap => NormalObjectPostion::Middle,
            _ => match self.pos {
                NormalObjectPostion::Middle => NormalObjectPostion::Leaf,
                NormalObjectPostion::Leaf | NormalObjectPostion::Assoc => {
                    NormalObjectPostion::Assoc
                }
            },
        };

        let path = match key {
            Some(key) => Self::dervie_path(&self.path, key),
            None => self.path.clone(),
        };

        let ref_depth = if is_ref_object {
            self.ref_depth + 1
        } else {
            self.ref_depth
        };

        Self {
            pos,
            object: NONSlimObjectInfo::new(object_id, None, None),
            path,
            config_ref_depth: self.config_ref_depth,
            ref_depth,
        }
    }
}

#[derive(Debug)]
pub struct SubObject {
    pub object_id: ObjectId,
}

#[derive(Debug)]
pub enum TraverseObjectItem {
    Normal(NormalObject),
    Sub(SubObject),
}

impl TraverseObjectItem {
    pub fn object_id(&self) -> &ObjectId {
        match self {
            Self::Normal(item) => &item.object.object_id,
            Self::Sub(item) => &item.object_id,
        }
    }
}

pub struct TraverseChunkItem {
    pub chunk_id: ChunkId,
}

#[async_trait::async_trait]
pub trait ObjectTraverserCallBack: Send + Sync {
    async fn on_object(&self, item: TraverseObjectItem) -> BuckyResult<()>;
    async fn on_chunk(&self, item: TraverseChunkItem) -> BuckyResult<()>;

    async fn on_error(&self, id: &ObjectId, e: BuckyError) -> BuckyResult<()>;
    async fn on_missing(&self, id: &ObjectId) -> BuckyResult<()>;
}

pub type ObjectTraverserCallBackRef = Arc<Box<dyn ObjectTraverserCallBack>>;
