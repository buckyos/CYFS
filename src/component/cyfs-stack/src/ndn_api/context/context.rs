use cyfs_bdt::*;
use cyfs_base::*;
use cyfs_core::TransContextObject;
use super::manager::ContextManagerRef;

use std::collections::LinkedList;
use std::sync::Arc;

pub struct TransContextHolderInner {
    id: ObjectId,
    referer: String,
    manager: ContextManagerRef,
}

impl TransContextHolderInner {
    async fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool {
        let ret = self.manager.get_context(&self.id).await;
        if ret.is_none() {
            return false;
        }

        let context = ret.unwrap();
        let ret = context.object.device_list().iter().find(|item| {
            if item.target.eq(target) && item.chunk_codec_type.support_desc(encode_desc)  {
                true
            } else {
                false
            }
        });

        ret.is_some()
    }

    async fn sources_of(
        &self,
        filter: Box<dyn Fn(&DownloadSource) -> bool>,
        limit: usize,
    ) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let ret = self.manager.get_context(&self.id).await;
        if ret.is_none() {
            return result;
        }

        let context = ret.unwrap();
        let mut count = 0;
        for source in &context.source_list {
            if (*filter)(source) {
                result.push_back(source.clone());
                count += 1;
                if count >= limit {
                    return result;
                }
            }
        }

        result
    }
}

#[derive(Clone)]
pub struct TransContextHolder(Arc<TransContextHolderInner>);

impl DownloadContext for TransContextHolder {
    fn clone_as_context(&self) -> Box<dyn DownloadContext> {
        Box::new(self.clone())
    }

    fn referer(&self) -> &str {
        &self.0.referer
    }

    fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool {
        todo!();
    }

    fn sources_of(
        &self,
        filter: Box<dyn Fn(&DownloadSource) -> bool>,
        limit: usize,
    ) -> LinkedList<DownloadSource> {
        todo!();
    }
}