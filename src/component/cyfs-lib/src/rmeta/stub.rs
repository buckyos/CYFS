use super::def::*;
use super::output_request::*;
use super::processor::*;
use super::request::*;
use cyfs_base::*;

pub struct GlobalStateMetaStub {
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,
    processor: GlobalStateMetaOutputProcessorRef,
}

impl GlobalStateMetaStub {
    pub fn new(
        processor: GlobalStateMetaOutputProcessorRef,
        target: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> Self {
        Self {
            processor,
            target,
            dec_id,
        }
    }

    // path access
    pub async fn add_access(&self, item: GlobalStatePathAccessItem) -> BuckyResult<bool> {
        let req = GlobalStateMetaAddAccessRequest {
            common: MetaOutputRequestCommon {
                dec_id: self.dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            item,
        };

        let resp = self.processor.add_access(req).await?;
        Ok(resp.updated)
    }

    pub async fn remove_access(
        &self,
        item: GlobalStatePathAccessItem,
    ) -> BuckyResult<Option<GlobalStatePathAccessItem>> {
        let req = GlobalStateMetaRemoveAccessRequest {
            common: MetaOutputRequestCommon {
                dec_id: self.dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            item,
        };

        let resp = self.processor.remove_access(req).await?;
        Ok(resp.item)
    }

    pub async fn clear_access(&self) -> BuckyResult<u32> {
        let req = GlobalStateMetaClearAccessRequest {
            common: MetaOutputRequestCommon {
                dec_id: self.dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
        };

        let resp = self.processor.clear_access(req).await?;
        Ok(resp.count)
    }

    pub async fn add_link(
        &self,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> BuckyResult<bool> {
        let req = GlobalStateMetaAddLinkRequest {
            common: MetaOutputRequestCommon {
                dec_id: self.dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            source: source.into(),
            target: target.into(),
        };

        let resp = self.processor.add_link(req).await?;
        Ok(resp.updated)
    }

    pub async fn remove_link(
        &self,
        source: impl Into<String>,
    ) -> BuckyResult<Option<GlobalStatePathLinkItem>> {
        let req = GlobalStateMetaRemoveLinkRequest {
            common: MetaOutputRequestCommon {
                dec_id: self.dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            source: source.into(),
        };

        let resp = self.processor.remove_link(req).await?;
        Ok(resp.item)
    }

    pub async fn clear_link(&self) -> BuckyResult<u32> {
        let req = GlobalStateMetaClearLinkRequest {
            common: MetaOutputRequestCommon {
                dec_id: self.dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
        };

        let resp = self.processor.clear_link(req).await?;
        Ok(resp.count)
    }
}
