use super::def::*;
use cyfs_base::*;

pub struct FileObjectTraverser {
    current: NormalObject,
    cb: ObjectTraverserCallBackRef,
}

impl FileObjectTraverser {
    pub fn new(current: NormalObject, cb: ObjectTraverserCallBackRef) -> Self {
        Self { current, cb }
    }

    pub fn finish(self) -> NormalObject {
        self.current
    }

    pub async fn tranverse(&self) {
        let object = self.current.object.object.as_ref().unwrap();
        let file = object.as_file();

        match file.body() {
            Some(body) => match body.content().inner_chunk_list() {
                Some(list) => {
                    for chunk_id in list.iter() {
                        self.append_chunk(chunk_id).await;
                    }
                }
                None => {}
            },
            None => {}
        }
    }

    async fn append_chunk(&self, chunk_id: &ChunkId) {
        let item = TraverseChunkItem {
            chunk_id: chunk_id.to_owned(),
        };

        self.cb.on_chunk(item).await;
    }
}