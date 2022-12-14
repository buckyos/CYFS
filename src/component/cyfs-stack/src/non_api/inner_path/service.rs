use super::dir_loader::*;
use super::objectmap_loader::*;
use crate::NamedDataComponents;
use crate::ndn_api::LocalDataManager;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// Used to support dir+innerpath and objectmap+innerpath modes
pub(crate) struct NONInnerPathServiceProcessor {
    next: NONInputProcessorRef,

    dir_loader: NONDirLoader,
    objectmap_loader: NONObjectMapLoader,
}

impl NONInnerPathServiceProcessor {
    pub fn new(
        non_processor: NONInputProcessorRef,
        named_data_components: &NamedDataComponents,
        noc: NamedObjectCacheRef,
    ) -> NONInputProcessorRef {
        let data_manager = LocalDataManager::new(named_data_components);
        let dir_loader = NONDirLoader::new(non_processor.clone(), data_manager);

        // TODO objectmap loader should use non instead noc?
        let objectmap_loader = NONObjectMapLoader::new(noc);

        let ret = Self {
            next: non_processor,
            dir_loader,
            objectmap_loader,
        };

        Arc::new(Box::new(ret))
    }

    async fn get_objectmap(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let ret = self.objectmap_loader.load(req).await?;

        let mut resp = NONGetObjectInputResponse::new_with_object(ret);
        resp.init_times()?;
        Ok(resp)
    }

    async fn get_dir(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let ret = self.dir_loader.get_dir(req).await?;
        let resp = match ret {
            DirResult::File((file, attr)) => {
                let object_raw = file.to_vec()?;
                let object = AnyNamedObject::Standard(StandardObject::File(file));

                let mut resp = NONGetObjectInputResponse::new(
                    object.object_id(),
                    object_raw,
                    Some(Arc::new(object)),
                );
                resp.attr = Some(attr);
                resp.init_times()?;
                resp
            }
            DirResult::Dir((dir, attr)) => {
                let object_raw = dir.to_vec()?;
                let object = AnyNamedObject::Standard(StandardObject::Dir(dir));

                let mut resp = NONGetObjectInputResponse::new(
                    object.object_id(),
                    object_raw,
                    Some(Arc::new(object)),
                );
                resp.attr = Some(attr);
                resp.init_times()?;
                resp
            }
        };

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONInnerPathServiceProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        // 对objectmap+inner_path情况下
        if req.object_id.obj_type_code() == ObjectTypeCode::ObjectMap && req.inner_path.is_some() {
            return self.get_objectmap(req).await;
        }

        // 对dir+innerpath情况下
        if req.object_id.obj_type_code() == ObjectTypeCode::Dir && req.inner_path.is_some() {
            return self.get_dir(req).await;
        }

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        self.next.delete_object(req).await
    }
}
