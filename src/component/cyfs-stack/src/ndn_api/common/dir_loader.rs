use super::super::data::ChunkStoreReader;
use cyfs_base::*;

use async_std::io::ReadExt;
use std::borrow::Cow;

pub(crate) struct DirLoader {
    chunk_reader: ChunkStoreReader,
}

impl DirLoader {
    pub fn new(chunk_reader: ChunkStoreReader) -> Self {
        Self { chunk_reader }
    }

    pub async fn load_desc_obj_list<'a>(
        &self,
        dir_id: &ObjectId,
        dir: &'a Dir,
    ) -> BuckyResult<Cow<'a, NDNObjectList>> {
        let obj_list = match &dir.desc().content().obj_list() {
            NDNObjectInfo::Chunk(id) => {
                let body = self.load_body_obj_list(dir_id, dir).await?;
                let list = self
                    .load_from_body_and_chunk_manager(dir_id, &id, &body)
                    .await?;
                Cow::Owned(list)
            }
            NDNObjectInfo::ObjList(list) => Cow::Borrowed(list),
        };

        Ok(obj_list)
    }

    pub async fn load_desc_and_body<'a>(
        &self,
        dir_id: &ObjectId,
        dir: &'a Dir,
    ) -> BuckyResult<(
        Cow<'a, NDNObjectList>,
        Option<Cow<'a, DirBodyContentObjectList>>,
    )> {
        let body = self.load_body_obj_list(dir_id, dir).await?;

        let obj_list = match &dir.desc().content().obj_list() {
            NDNObjectInfo::Chunk(id) => {
                let list = self
                    .load_from_body_and_chunk_manager(dir_id, &id, &body)
                    .await?;
                Cow::Owned(list)
            }
            NDNObjectInfo::ObjList(list) => Cow::Borrowed(list),
        };

        Ok((obj_list, body))
    }

    async fn load_from_body_and_chunk_manager<'a, T: for<'de> RawDecode<'de>>(
        &self,
        dir_id: &ObjectId,
        chunk_id: &ChunkId,
        body: &Option<Cow<'a, DirBodyContentObjectList>>,
    ) -> BuckyResult<T> {
        // first try to load chunk from body
        if let Some(body) = body {
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
        let mut reader = self
            .chunk_reader
            .get_chunk(chunk_id)
            .await
            .map_err(|e| {
                error!(
                    "load dir desc chunk error! dir={}, chunk={}, {}",
                    dir_id, chunk_id, e,
                );
                e
            })?;

        let mut buf = vec![];
        let read_len = reader.read_to_end(&mut buf).await.map_err(|e| {
            let msg = format!(
                "load dir related chunk to buf error! dir={}, chunk={}, {}",
                dir_id, chunk_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if read_len != chunk_id.len() {
            let msg = format!(
                "load dir related chunk to buf but len unmatch! dir={}, chunk={}, read={}, len={}",
                dir_id, chunk_id, read_len, chunk_id.len(),
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        let (ret, _) = T::raw_decode(&buf)?;
        Ok(ret)
    }
}
