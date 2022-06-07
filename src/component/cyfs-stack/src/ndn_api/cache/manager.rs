use cyfs_base::*;
use cyfs_bdt::{ChunkWriter, StackGuard};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use super::writer::{LocalChunkWriter, LocalFileWriter};

struct NDNDataCacheManagerInner {
    bdt_stack: StackGuard,
    file_root: PathBuf,
    chunk_root: PathBuf,
}

impl NDNDataCacheManagerInner {
    pub fn new(bdt_stack: StackGuard, isolate: &str) -> Self {
        let mut root = cyfs_util::get_named_data_root(isolate);
        root.push("cache");

        Self {
            bdt_stack,
            file_root: Self::create_root(&root, "file"),
            chunk_root: Self::create_root(&root, "chunk"),
        }
    }

    fn create_root(root: &Path, name: &str) -> PathBuf {
        let root = root.join(name);
        if !root.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&root) {
                error!(
                    "create data cache dir failed! dir={}, err={}",
                    root.display(),
                    e
                );
            } else {
                info!("create dat cache dir success! {}", root.display());
            }
        }

        root
    }

    pub async fn gen_file_writer(
        &self,
        file_id: &ObjectId,
        file: &File,
    ) -> BuckyResult<Option<Box<dyn ChunkWriter>>> {
        // 使用基于file_id的缓存，如果同名文件存在的话，就不再需要缓存
        let file_id_str = file_id.to_string();
        let file_path = self.file_root.join(&file_id_str);
        if file_path.exists() {
            info!("cache file already exists! file={}", file_id_str);
            return Ok(None);
        }

        // let chunk_list = ChunkListDesc::from_file(&file)?;
        // let writer = self
        //     .bdt_stack
        //     .ndn()
        //     .chunk_manager()
        //     .store()
        //     .file_writer(&file_path, &chunk_list);

        let writer = LocalFileWriter::new(file_path.clone(),
                                          file.clone(),
                                          self.bdt_stack.ndn().chunk_manager().ndc().clone(),
                                          self.bdt_stack.ndn().chunk_manager().tracker().clone()).await?;
        info!(
            "will cache file: {} => {}, len={}",
            file_id_str,
            file_path.display(),
            file.desc().content().len()
        );
        Ok(Some(Box::new(writer)))
    }

    pub async fn gen_chunk_writer(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<Box<dyn ChunkWriter>>> {
        // 使用基于chunk_id的缓存，如果同名文件存在的话，就不再需要缓存
        let chunk_id_str = chunk_id.to_string();
        let file_path = self.chunk_root.join(&chunk_id_str);
        if file_path.exists() {
            info!("cache chunk already exists! file={}", chunk_id_str);
            return Ok(None);
        }

        // let chunk_list = ChunkListDesc::from_chunk(chunk_id.to_owned());
        // let writer = self
        //     .bdt_stack
        //     .ndn()
        //     .chunk_manager()
        //     .store()
        //     .file_writer(&file_path, &chunk_list);

        let writer = LocalChunkWriter::new(file_path.clone(),
                                           self.bdt_stack.ndn().chunk_manager().ndc().clone(),
                                           self.bdt_stack.ndn().chunk_manager().tracker().clone());
        info!(
            "will cache chunk: {} => {}",
            chunk_id_str,
            file_path.display()
        );
        Ok(Some(Box::new(writer)))
    }
}

#[derive(Clone)]
pub(crate) struct NDNDataCacheManager(Arc<NDNDataCacheManagerInner>);

impl NDNDataCacheManager {
    pub(crate) fn new(bdt_stack: StackGuard, isolate: &str) -> Self {
        let ret = NDNDataCacheManagerInner::new(bdt_stack, isolate);
        Self(Arc::new(ret))
    }

    pub async fn gen_file_writer(
        &self,
        file_id: &ObjectId,
        file: &File,
    ) -> BuckyResult<Option<Box<dyn ChunkWriter>>> {
        self.0.gen_file_writer(file_id, file).await
    }

    pub async fn gen_chunk_writer(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<Box<dyn ChunkWriter>>> {
        self.0.gen_chunk_writer(chunk_id).await
    }
}
