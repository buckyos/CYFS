use super::def::*;
use super::output_request::*;
use super::processor::*;
use super::request::*;
use cyfs_base::*;

pub struct GlobalStateMetaStub {
    target: Option<ObjectId>,
    target_dec_id: Option<ObjectId>,
    processor: GlobalStateMetaOutputProcessorRef,
}

impl GlobalStateMetaStub {
    pub fn new(
        processor: GlobalStateMetaOutputProcessorRef,
        target: Option<ObjectId>,
        target_dec_id: Option<ObjectId>,
    ) -> Self {
        Self {
            processor,
            target,
            target_dec_id,
        }
    }

    // path access
    pub async fn add_access(&self, item: GlobalStatePathAccessItem) -> BuckyResult<bool> {
        let req = GlobalStateMetaAddAccessRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
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
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
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
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
        };

        let resp = self.processor.clear_access(req).await?;
        Ok(resp.count)
    }

    // path link
    pub async fn add_link(
        &self,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> BuckyResult<bool> {
        let req = GlobalStateMetaAddLinkRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
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
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
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
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
        };

        let resp = self.processor.clear_link(req).await?;
        Ok(resp.count)
    }

    // object meta
    pub async fn add_object_meta(&self, item: GlobalStateObjectMetaItem) -> BuckyResult<bool> {
        let req = GlobalStateMetaAddObjectMetaRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            item,
        };

        let resp = self.processor.add_object_meta(req).await?;
        Ok(resp.updated)
    }

    pub async fn remove_object_meta(
        &self,
        item: GlobalStateObjectMetaItem,
    ) -> BuckyResult<Option<GlobalStateObjectMetaItem>> {
        let req = GlobalStateMetaRemoveObjectMetaRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            item,
        };

        let resp = self.processor.remove_object_meta(req).await?;
        Ok(resp.item)
    }

    pub async fn clear_object_meta(&self) -> BuckyResult<u32> {
        let req = GlobalStateMetaClearObjectMetaRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
        };

        let resp = self.processor.clear_object_meta(req).await?;
        Ok(resp.count)
    }

    // path config
    pub async fn add_path_config(&self, item: GlobalStatePathConfigItem) -> BuckyResult<bool> {
        let req = GlobalStateMetaAddPathConfigRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            item,
        };

        let resp = self.processor.add_path_config(req).await?;
        Ok(resp.updated)
    }

    pub async fn remove_path_config(
        &self,
        item: GlobalStatePathConfigItem,
    ) -> BuckyResult<Option<GlobalStatePathConfigItem>> {
        let req = GlobalStateMetaRemovePathConfigRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
            item,
        };

        let resp = self.processor.remove_path_config(req).await?;
        Ok(resp.item)
    }

    pub async fn clear_path_config(&self) -> BuckyResult<u32> {
        let req = GlobalStateMetaClearPathConfigRequest {
            common: MetaOutputRequestCommon {
                dec_id: None,
                target_dec_id: self.target_dec_id.clone(),
                target: self.target.clone(),
                flags: 0,
            },
        };

        let resp = self.processor.clear_path_config(req).await?;
        Ok(resp.count)
    }
}
