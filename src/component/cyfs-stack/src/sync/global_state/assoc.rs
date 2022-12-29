use super::data::ChunksCollector;
use cyfs_base::*;
use cyfs_lib::*;

use std::collections::HashSet;

pub(super) struct AssociationObjects {
    list: HashSet<ObjectId>,
    chunks_collector: ChunksCollector,
}

impl AssociationObjects {
    pub fn new(chunks_collector: ChunksCollector,) -> Self {
        Self {
            list: HashSet::new(),
            chunks_collector,
        }
    }

    pub fn append(&mut self, info: &NONObjectInfo) {
        // debug!("add object's assoc items: {}", info.object_id);
        let object = info.object.as_ref().unwrap();

        if let Some(id) = object.owner() {
            self.append_item(id);
        }

        if let Some(id) = object.prev() {
            self.append_item(id);
        }

        if let Some(id) = object.author() {
            self.append_item(id);
        }

        if let Some(dec_id) = object.dec_id() {
            self.append_item(dec_id);
        }

        if let Some(ref_list) = object.ref_objs() {
            for link in ref_list {
                self.append_link(link);
            }
        }

        if let Some(signs) = object.signs() {
            if let Some(signs) = signs.body_signs() {
                signs.iter().for_each(|sign| {
                    self.append_sign(sign);
                })
            }
            if let Some(signs) = signs.desc_signs() {
                signs.iter().for_each(|sign| {
                    self.append_sign(sign);
                })
            }
        }
    }

    fn append_sign(&mut self, sign: &Signature) {
        match sign.sign_source() {
            SignatureSource::Object(link) => {
                self.append_link(link);
            }
            _ => {}
        }
    }

    fn append_link(&mut self, link: &ObjectLink) {
        self.append_item(&link.obj_id);
        if let Some(ref owner) = link.obj_owner {
            self.append_item(&owner);
        }
    }

    pub fn append_item(&mut self, id: &ObjectId) {
        match id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                self.chunks_collector.append_chunk(id.as_chunk_id());
            }
            _ => {
                if id.is_data() {
                    return;
                }
                
                if self.list.get(id).is_none() {
                    self.list.insert(id.to_owned());
                }
            }
        }
    }

    pub fn into_list(self) -> Vec<ObjectId> {
        self.list.into_iter().collect()
    }
}
