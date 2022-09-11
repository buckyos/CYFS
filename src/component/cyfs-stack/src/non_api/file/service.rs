use super::dir_loader::*;
use super::objectmap_loader::*;
use crate::ndn::*;
use crate::non::*;
use crate::resolver::OodResolver;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NONFileServiceProcessor {
    next: NONInputProcessorRef,

    dir_loader: NONDirLoader,
    objectmap_loader: NONObjectMapLoader,
}

impl NONFileServiceProcessor {
    pub fn new(
        non_api_level: NONAPILevel,
        non_processor: NONInputProcessorRef,
        ndn_processor: NDNInputProcessorRef,
        ood_resolver: OodResolver,
        noc: NamedObjectCacheRef,
    ) -> NONInputProcessorRef {
        let objectmap_loader = NONObjectMapLoader::new(ood_resolver.device_id().clone(), noc);

        let dir_loader = NONDirLoader::new(
            non_api_level,
            non_processor.clone(),
            ndn_processor,
            ood_resolver,
        );

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
        let ret = self
            .objectmap_loader
            .load(&req.object_id, &req.inner_path.as_ref().unwrap())
            .await?;

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
impl NONInputProcessor for NONFileServiceProcessor {
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
