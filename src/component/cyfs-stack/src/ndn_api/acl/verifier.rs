use super::super::common::DirLoader;
use cyfs_bdt_ext::ChunkStoreReader;
use cyfs_base::*;

pub(crate) struct NDNRefererVerifier {
    dir: DirVerifier,
}

impl NDNRefererVerifier {
    pub fn new(chunk_reader: ChunkStoreReader) -> Self {
        Self {
            dir: DirVerifier::new(chunk_reader),
        }
    }

    pub async fn verify_referer(
        &self,
        obj_id: &ObjectId,
        obj: &AnyNamedObject,
        target_chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        match obj.obj_type_code() {
            ObjectTypeCode::File => {
                let file = obj.as_file();
                FileVerifier::verify(obj_id, file, target_chunk_id).await
            }
            ObjectTypeCode::Dir => {
                let dir = obj.as_dir();
                self.dir.verify(obj_id, dir, target_chunk_id).await
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

struct FileVerifier;

impl FileVerifier {
    pub async fn verify(
        file_id: &ObjectId,
        file: &File,
        target_chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
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
            Ok(())
        } else {
            let msg = format!(
                "target chunk is not found in file's chunk list! dir={}, target_chunk={}",
                file_id, target_chunk_id
            );
            warn!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
        }
    }
}
struct DirVerifier {
    dir_loader: DirLoader,
}

impl DirVerifier {
    pub fn new(chunk_reader: ChunkStoreReader) -> Self {
        Self {
            dir_loader: DirLoader::new(chunk_reader),
        }
    }

    pub async fn verify(
        &self,
        dir_id: &ObjectId,
        dir: &Dir,
        target_chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        let obj_list = self.dir_loader.load_desc_obj_list(dir_id, dir).await?;

        if let Some(parent_chunk) = &obj_list.parent_chunk {
            if parent_chunk == target_chunk_id {
                return Ok(());
            }
        }

        let ret = obj_list.object_map.iter().find(|(_k, v)| match v.node() {
            InnerNode::Chunk(id) => id == target_chunk_id,
            InnerNode::IndexInParentChunk(_, _) => false,
            InnerNode::ObjId(id) => id == target_chunk_id.as_object_id(),
        });

        if ret.is_some() {
            info!(
                "target chunk is exists in dir's obj list! dir={}, target_chunk={}",
                dir_id, target_chunk_id
            );
            Ok(())
        } else {
            let msg = format!(
                "target chunk is not found in dir's obj list! dir={}, target_chunk={}",
                dir_id, target_chunk_id
            );
            warn!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
        }
    }
}
