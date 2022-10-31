use crate::ndn_api::LocalDataManager;
use cyfs_base::*;
use cyfs_lib::*;

use std::borrow::Cow;

pub(crate) struct NDNChunkVerifier {
    dir: DirVerifier,
}

impl NDNChunkVerifier {
    pub fn new(data_manager: LocalDataManager) -> Self {
        Self {
            dir: DirVerifier::new(data_manager),
        }
    }

    pub async fn verify_chunk(
        &self,
        obj_id: &ObjectId,
        obj: &AnyNamedObject,
        target_chunk_id: &ChunkId,
    ) -> BuckyResult<bool> {
        match obj.obj_type_code() {
            ObjectTypeCode::File => {
                let file = obj.as_file();
                FileVerifier::verify(obj_id, file, target_chunk_id).await
            }
            ObjectTypeCode::Dir => {
                let dir = obj.as_dir();
                self.dir
                    .verify(obj_id, dir, target_chunk_id)
                    .await
            }
            _ => {
                let msg = format!(
                    "ndn verify chunk but unsupport object type: id={}, type={:?}, target_chunk={}",
                    obj_id,
                    obj.obj_type(),
                    target_chunk_id
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }
}

pub(crate) struct FileVerifier;

impl FileVerifier {
    pub async fn verify(
        file_id: &ObjectId,
        file: &File,
        target_chunk_id: &ChunkId,
    ) -> BuckyResult<bool> {
        let ret = match file.body() {
            Some(body) => match body.content().inner_chunk_list() {
                Some(list) => list.contains(&target_chunk_id),
                None => false,
            },
            None => false,
        };

        if ret {
            info!(
                "target chunk is exists in file's chunk list! dir={}, target_chunk={}",
                file_id, target_chunk_id
            );
            Ok(true)
        } else {
            warn!(
                "target chunk is not found in file's chunk list! dir={}, target_chunk={}",
                file_id, target_chunk_id
            );
            Ok(false)
        }
    }
}
pub(crate) struct DirVerifier {
    data_manager: LocalDataManager,
}

impl DirVerifier {
    pub fn new(data_manager: LocalDataManager) -> Self {
        Self { data_manager }
    }

    pub async fn verify(
        &self,
        dir_id: &ObjectId,
        dir: &Dir,
        target_chunk_id: &ChunkId,
    ) -> BuckyResult<bool> {
        let obj_list = self.load_desc_obj_list(dir_id, dir).await?;

        if let Some(parent_chunk) = &obj_list.parent_chunk {
            if parent_chunk == target_chunk_id {
                return Ok(true);
            }
        }

        let ret = obj_list.object_map.iter().find(|(k, v)| match v.node() {
            InnerNode::Chunk(id) => id == target_chunk_id,
            InnerNode::IndexInParentChunk(_, _) => false,
            InnerNode::ObjId(id) => id == target_chunk_id.as_object_id(),
        });

        if ret.is_some() {
            info!(
                "target chunk is exists in dir's obj list! dir={}, target_chunk={}",
                dir_id, target_chunk_id
            );
            Ok(true)
        } else {
            warn!(
                "target chunk is not found in dir's obj list! dir={}, target_chunk={}",
                dir_id, target_chunk_id
            );
            Ok(false)
        }
    }

    async fn load_desc_obj_list<'a>(
        &self,
        dir_id: &ObjectId,
        dir: &'a Dir,
    ) -> BuckyResult<Cow<'a, NDNObjectList>> {
        let obj_list = match &dir.desc().content().obj_list() {
            NDNObjectInfo::Chunk(id) => {
                let list: NDNObjectList = self
                    .load_from_body_and_chunk_manager(dir_id, dir, &id)
                    .await?;
                Cow::Owned(list)
            }
            NDNObjectInfo::ObjList(list) => Cow::Borrowed(list),
        };

        Ok(obj_list)
    }

    async fn load_from_body_and_chunk_manager<T: for<'a> RawDecode<'a>>(
        &self,
        dir_id: &ObjectId,
        dir: &Dir,
        chunk_id: &ChunkId,
    ) -> BuckyResult<T> {
        // first try to load chunk from body
        let ret = self.load_body_obj_list(dir_id, dir).await?;
        if let Some(body) = ret {
            let ret = body.get(chunk_id.as_object_id());
            if ret.is_some() {
                debug!(
                    "load chunk from dir body! dir={}, chunk={}",
                    dir_id, chunk_id
                );
                let buf = ret.unwrap();
                let (ret, _) = T::raw_decode(&buf)?;
                return Ok(ret);
            }
        }

        // then try to load chunk from chunk manager
        self.load_from_chunk_manager(dir_id, chunk_id).await
    }

    async fn load_body_obj_list<'a>(
        &self,
        dir_id: &ObjectId,
        dir: &'a Dir,
    ) -> BuckyResult<Option<Cow<'a, DirBodyContentObjectList>>> {
        let ret = match dir.body() {
            Some(body) => {
                let list = match body.content() {
                    DirBodyContent::Chunk(id) => {
                        let list: DirBodyContentObjectList =
                            self.load_from_chunk_manager(dir_id, id).await?;
                        Cow::Owned(list)
                    }
                    DirBodyContent::ObjList(list) => Cow::Borrowed(list),
                };

                Some(list)
            }
            None => None,
        };

        Ok(ret)
    }

    async fn load_from_chunk_manager<T: for<'a> RawDecode<'a>>(
        &self,
        dir_id: &ObjectId,
        chunk_id: &ChunkId,
    ) -> BuckyResult<T> {
        let ret = self.data_manager.get_chunk(chunk_id, None).await;
        if ret.is_err() {
            error!(
                "load dir desc chunk error! dir={}, chunk={}, {}",
                dir_id,
                chunk_id,
                ret.as_ref().unwrap_err()
            );

            return ret;
        }

        let ret = ret.unwrap();
        if ret.is_none() {
            let msg = format!(
                "load dir desc chunk but not found! dir={}, chunk={}, {}",
                dir_id, chunk_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (reader, len) = ret.unwrap();
        let mut buf = vec![];
        reader.read_to_end(&mut buf).map_err(|e| {
            let msg = format!(
                "load dir desc chunk to buf error! dir={}, chunk={}, {}",
                dir_id, chunk_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let (ret, _) = T::raw_decode(&buf)?;
        Ok(ret)
    }
}
