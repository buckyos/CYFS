use super::def::*;
use cyfs_base::*;

pub struct CommonObjectTraverser {
    current: NormalObject,
    cb: ObjectTraverserCallBackRef,
}

impl CommonObjectTraverser {
    pub fn new(current: NormalObject, cb: ObjectTraverserCallBackRef) -> Self {
        Self { current, cb }
    }

    pub fn finish(self) -> NormalObject {
        self.current
    }

    pub async fn tranverse(&self) -> BuckyResult<()> {
        let object = self.current.object.object.as_ref().unwrap();
        if let Some(id) = object.owner() {
            self.append_item(id).await?;
        }

        if let Some(id) = object.prev() {
            self.append_item(id).await?;
        }

        if let Some(id) = object.author() {
            self.append_item(id).await?;
        }

        if let Some(dec_id) = object.dec_id() {
            self.append_item(dec_id).await?;
        }

        if let Some(ref_list) = object.ref_objs() {
            for link in ref_list {
                self.append_link(link).await?;
            }
        }

        if let Some(signs) = object.signs() {
            if let Some(signs) = signs.body_signs() {
                for sign in signs.iter() {
                    self.append_sign(sign).await?;
                }
            }
            if let Some(signs) = signs.desc_signs() {
                for sign in signs.iter() {
                    self.append_sign(sign).await?;
                }
            }
        }

        Ok(())
    }

    async fn append_sign(&self, sign: &Signature) -> BuckyResult<()> {
        match sign.sign_source() {
            SignatureSource::Object(link) => self.append_link(link).await,
            _ => Ok(()),
        }
    }

    async fn append_link(&self, link: &ObjectLink) -> BuckyResult<()> {
        self.append_item(&link.obj_id).await?;

        if let Some(ref owner) = link.obj_owner {
            self.append_item(&owner).await?;
        }

        Ok(())
    }

    async fn append_item(&self, id: &ObjectId) -> BuckyResult<()> {
        match id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                let item = TraverseChunkItem {
                    chunk_id: id.as_chunk_id().to_owned(),
                };
                self.cb.on_chunk(item).await
            }
            _ => {
                if id.is_data() {
                    return Ok(());
                }

                let obj = self.current.derive_normal(id.to_owned(), None, true);
                let item = TraverseObjectItem::Normal(obj);
                self.cb.on_object(item).await
            }
        }
    }
}
