use super::super::traverser::ObjectTraverserLoaderRef;
use super::def::*;
use cyfs_base::*;

use async_std::io::prelude::*;
use std::borrow::Cow;

pub struct DirObjectTraverser {
    loader: ObjectTraverserLoaderRef,
    current: NormalObject,
    cb: ObjectTraverserCallBackRef,

    body_obj_list: Option<DirBodyContentObjectList>,
}

impl DirObjectTraverser {
    pub fn new(
        loader: ObjectTraverserLoaderRef,
        current: NormalObject,
        cb: ObjectTraverserCallBackRef,
    ) -> Self {
        Self {
            loader,
            current,
            cb,
            body_obj_list: None,
        }
    }

    fn dir_id(&self) -> &ObjectId {
        &self.current.object.object_id
    }

    fn dir(&self) -> &Dir {
        let object = self.current.object.object.as_ref().unwrap();
        object.as_dir()
    }

    pub async fn tranverse(&mut self) -> BuckyResult<()> {
        // First init body chunk
        if let Err(e) = self.load_body_chunk().await {
            self.cb.on_error(self.dir_id(), e).await?;
            return Ok(());
        }

        // Init desc obj list
        let dir = self.dir();
        let desc_obj_list = match dir.desc().content().obj_list() {
            NDNObjectInfo::Chunk(chunk_id) => {
                match self.load_chunk_from_body_and_reader(chunk_id).await {
                    Ok(chunk) => {
                        match &chunk {
                            Cow::Owned(_) => {
                                let item = TraverseChunkItem {
                                    chunk_id: chunk_id.to_owned(),
                                };
                                self.cb.on_chunk(item).await?;
                            }
                            Cow::Borrowed(_) => {}
                        }

                        match NDNObjectList::clone_from_slice(&chunk) {
                            Ok(obj_list) => Cow::Owned(obj_list),
                            Err(e) => {
                                let msg = format!("decode dir desc chunk to obj list failed! dir={}, chunk={}, {}", self.current.object.object_id, chunk_id, e);
                                error!("{}", msg);
                                let e = BuckyError::new(BuckyErrorCode::InvalidData, msg);
                                self.cb.on_error(self.dir_id(), e).await?;
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        self.cb.on_error(self.dir_id(), e).await?;
                        return Ok(());
                    }
                }
            }
            NDNObjectInfo::ObjList(list) => Cow::Borrowed(list),
        };

        self.append_desc_obj_list(&desc_obj_list).await
    }

    async fn load_body_chunk(&mut self) -> BuckyResult<()> {
        let dir = self.dir();

        if let Some(body) = dir.body() {
            match body.content() {
                DirBodyContent::Chunk(chunk_id) => {
                    let item = TraverseChunkItem {
                        chunk_id: chunk_id.to_owned(),
                    };
                    self.cb.on_chunk(item).await?;

                    match self.loader.get_chunk(chunk_id).await {
                        Ok(Some(mut reader)) => {
                            let mut buf = Vec::with_capacity(chunk_id.len());
                            reader.read_to_end(&mut buf).await.map_err(|e| {
                                let msg = format!(
                                    "read dir body's chunk to buf failed! dir={}, chunk={}, {}",
                                    self.dir_id(),
                                    chunk_id,
                                    e
                                );
                                error!("{}", msg);
                                let e: BuckyError = e.into();
                                BuckyError::new(e.code(), msg)
                            })?;

                            let obj_list = DirBodyContentObjectList::clone_from_slice(&buf).map_err(|e|{
                                let msg = format!(
                                    "decode dir body's chunk to object list failed! dir={}, chunk={}, {}",
                                    self.dir_id(),
                                    chunk_id,
                                    e
                                );
                                error!("{}", msg);
                                BuckyError::new(e.code(), msg)
                            })?;

                            assert!(self.body_obj_list.is_none());
                            self.body_obj_list = Some(obj_list);
                            Ok(())
                        }
                        Ok(None) => {
                            let msg = format!(
                                "get dir body's chunk but not found! dir={}, chunk={}",
                                self.dir_id(),
                                chunk_id
                            );
                            error!("{}", msg);
                            self.cb.on_missing(chunk_id.as_object_id()).await?;

                            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                        }
                        Err(e) => {
                            let msg = format!(
                                "get dir body's chunk failed! dir={}, chunk={}, {}",
                                self.dir_id(),
                                chunk_id,
                                e
                            );
                            error!("{}", msg);
                            Err(BuckyError::new(e.code(), msg))
                        }
                    }
                }
                DirBodyContent::ObjList(_list) => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    async fn load_chunk_from_body_and_reader<'a>(
        &'a self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Cow<'a, Vec<u8>>> {
        let dir = self.dir();
        let chunk = if let Some(body) = dir.body() {
            match body.content() {
                DirBodyContent::Chunk(chunk_id) => self
                    .body_obj_list
                    .as_ref()
                    .unwrap()
                    .get(chunk_id.as_object_id()),
                DirBodyContent::ObjList(list) => list.get(chunk_id.as_object_id()),
            }
        } else {
            None
        };

        if let Some(chunk) = chunk {
            return Ok(Cow::Borrowed(chunk));
        }

        // load from reader
        match self.loader.get_chunk(chunk_id).await {
            Ok(Some(mut reader)) => {
                let mut buf = Vec::with_capacity(chunk_id.len());
                reader.read_to_end(&mut buf).await.map_err(|e| {
                    let msg = format!(
                        "read dir desc's chunk to buf failed! dir={}, chunk={}, {}",
                        self.dir_id(),
                        chunk_id,
                        e
                    );
                    error!("{}", msg);
                    let e: BuckyError = e.into();
                    BuckyError::new(e.code(), msg)
                })?;

                Ok(Cow::Owned(buf))
            }
            Ok(None) => {
                let msg = format!(
                    "get dir body's chunk but not found! dir={}, chunk={}",
                    self.dir_id(),
                    chunk_id
                );
                error!("{}", msg);
                self.cb.on_missing(chunk_id.as_object_id()).await?;

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => {
                let msg = format!(
                    "get dir body's chunk failed! dir={}, chunk={}, {}",
                    self.dir_id(),
                    chunk_id,
                    e
                );
                error!("{}", msg);
                Err(BuckyError::new(e.code(), msg))
            }
        }
    }

    async fn append_desc_obj_list(&self, list: &NDNObjectList) -> BuckyResult<()> {
        let body_map = self.body_map();

        if let Some(ref parent) = list.parent_chunk {
            self.append_object(parent.as_object_id(), &body_map).await?;
        }

        for (_k, v) in &list.object_map {
            match v.node() {
                InnerNode::ObjId(id) => {
                    self.append_object(id, &body_map).await?;
                }
                InnerNode::Chunk(id) => {
                    self.append_object(id.as_object_id(), &body_map).await?;
                }
                InnerNode::IndexInParentChunk(_, _) => {}
            }
        }

        Ok(())
    }

    async fn append_object(
        &self,
        id: &ObjectId,
        body_map: &Option<&DirBodyContentObjectList>,
    ) -> BuckyResult<()> {
        if let Some(body_map) = body_map {
            if body_map.contains_key(id) {
                return Ok(());
            }
        }

        debug!(
            "dir assoc object not exists in body and local: dir={}, object={}",
            self.dir_id(),
            id
        );

        match id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                let item = TraverseChunkItem {
                    chunk_id: id.as_chunk_id().to_owned(),
                };

                self.cb.on_chunk(item).await
            }
            _ => {
                let item = self
                    .current
                    .derive_normal(self.dir_id().to_owned(), None, true);
                let item = TraverseObjectItem::Normal(item);
                self.cb.on_object(item).await
            }
        }
    }

    fn body_map(&self) -> Option<&DirBodyContentObjectList> {
        if let Some(body) = self.dir().body() {
            match body.content() {
                DirBodyContent::Chunk(_) => {
                    assert!(self.body_obj_list.is_some());
                    self.body_obj_list.as_ref()
                }
                DirBodyContent::ObjList(list) => Some(list),
            }
        } else {
            None
        }
    }
}
