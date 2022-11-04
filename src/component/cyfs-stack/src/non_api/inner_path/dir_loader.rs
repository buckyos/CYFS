use crate::ndn_api::DirLoader;
use crate::ndn_api::LocalDataManager;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::borrow::Cow;

use std::sync::Arc;

// dir+inner_path，对应一个sub_dir/file/单chunk文件
pub enum DirResult {
    Dir((Dir, Attributes)),
    File((File, Attributes)),
}

pub(crate) struct NONDirLoader {
    dir_loader: DirLoader,
    non: NONInputProcessorRef,
}

impl NONDirLoader {
    pub fn new(non: NONInputProcessorRef, data_manager: LocalDataManager) -> Self {
        Self {
            non,
            dir_loader: DirLoader::new(data_manager),
        }
    }

    // start with '/' and must not end with '/'
    fn check_fix_inner_path<'a>(
        dir_id: &ObjectId,
        inner_path: &'a str,
    ) -> BuckyResult<Cow<'a, str>> {
        let path = inner_path.trim();
        if path == "/" {
            let msg = format!(
                "get dir with invalid inner_path! dir={}, inner_path={}",
                dir_id, path
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let path = path.trim_end_matches('/');
        let ret = if path.starts_with('/') {
            Cow::Borrowed(path)
        } else {
            let path = format!("/{}", path);
            Cow::Owned(path)
        };

        Ok(ret)
    }

    pub async fn get_dir(&self, req: NONGetObjectInputRequest) -> BuckyResult<DirResult> {
        assert!(req.inner_path.is_some());
        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::Dir);

        info!("will get dir: {}", req);

        // first load root dir object from noc
        let ret = self
            .load_object_from_non(&req.common, &req.object_id)
            .await?;
        if ret.is_none() {
            let msg = format!("load dir from noc but not found! dir={}", req.object_id);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (obj, _obj_raw) = ret.unwrap();

        // load the root dir with full parts
        let (desc, body) = self
            .dir_loader
            .load_desc_and_body(&req.object_id, obj.as_dir())
            .await?;

        // find target with inner_path, only support zip mode!
        let inner_path =
            Self::check_fix_inner_path(&req.object_id, req.inner_path.as_ref().unwrap())?;

        let mut ret = desc.object_map.get(inner_path.as_ref());
        if ret.is_none() {
            // remove the leading '/' then have another retry
            let inner_path2 = inner_path.trim_start_matches('/');
            ret = desc.object_map.get(inner_path2);
            if ret.is_none() {
                let msg = format!(
                    "load dir with inner_path but target not found! dir={}, inner_path={}",
                    req.object_id, inner_path,
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        }

        let info = ret.unwrap();

        let ret = loop {
            match info.node() {
                InnerNode::ObjId(object_id) => {
                    if object_id.obj_type_code() != ObjectTypeCode::File {
                        let msg = format!(
                            "dir inner node object type not support! dir={}, inner_path={}, obj={}, type={:?}",
                            req.object_id, inner_path, object_id, object_id.obj_type_code(),
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                    }

                    // first get from body
                    if let Some(body) = body {
                        let ret = body.get(object_id);
                        if ret.is_some() {
                            debug!(
                                "load object from dir body! dir={}, obj={}",
                                req.object_id, object_id
                            );
                            let buf = ret.unwrap();
                            let (file, _) = File::raw_decode(&buf).map_err(|e| {
                                let msg = format!("invalid file object data in dir body! dir={}, inner_path={}, file={}, {}", 
                                req.object_id, inner_path, object_id, e);
                                warn!("{}", msg);
                                BuckyError::new(BuckyErrorCode::InvalidData, msg)
                            })?;

                            break DirResult::File((file, info.attributes().to_owned()));
                        }
                    }

                    // then try load from noc
                    let ret = self.load_object_from_non(&req.common, &object_id).await?;
                    if ret.is_none() {
                        let msg = format!("load dir inner_path object from noc but not found! dir={}, inner_path={}, file={}", 
                            req.object_id, inner_path, object_id);
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                    }

                    let (file, _) = ret.unwrap();
                    let file: AnyNamedObject = file.into();
                    let file = file.into_file();

                    break DirResult::File((file, info.attributes().to_owned()));
                }
                InnerNode::Chunk(chunk_id) => {
                    // 单文件chunk
                    info!(
                        "got single chunk file! dir={}, inner_path={},  chunk={}",
                        req.object_id, inner_path, chunk_id,
                    );

                    // 创建一个单chunk的file对象
                    let builder = File::new_no_owner(
                        chunk_id.len() as u64,
                        HashValue::from(chunk_id.hash()),
                        ChunkList::ChunkInList(vec![chunk_id.to_owned()]),
                    );
                    let builder = builder.no_create_time();
                    let builder = if let Some(owner) = obj.as_dir().desc().owner() {
                        builder.owner(owner.to_owned())
                    } else {
                        builder
                    };
                    let file = builder.build();

                    break DirResult::File((file, info.attributes().to_owned()));
                }
                InnerNode::IndexInParentChunk(_, _) => {
                    let msg = format!(
                        "dir inner node type of IndexInParentChunk not support yet! dir={}, inner_path={}",
                        req.object_id, inner_path,
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                }
            }
        };

        Ok(ret)
    }

    async fn load_object_from_non(
        &self,
        common: &NONInputRequestCommon,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(Arc<AnyNamedObject>, Vec<u8>)>> {
        let get_req = NONGetObjectInputRequest {
            common: common.to_owned(),
            object_id: object_id.to_owned(),
            inner_path: None,
        };

        let ret = self.non.get_object(get_req).await;
        match ret {
            Ok(resp) => Ok(Some((resp.object.object.unwrap(), resp.object.object_raw))),
            Err(e) => match e.code() {
                BuckyErrorCode::NotFound => Ok(None),
                _ => {
                    error!("load object from non error! id={}, {}", object_id, e);
                    Err(e)
                }
            },
        }
    }
}
