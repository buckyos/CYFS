use cyfs_base::*;

pub struct SNDirParser;

impl SNDirParser {
    pub fn parse(id: Option<&ObjectId>, object_raw: &[u8]) -> BuckyResult<Vec<(DeviceId, Device)>> {
        let dir = Dir::clone_from_slice(&object_raw)?;
        let id = id.cloned().unwrap_or(dir.desc().calculate_id());

        let mut sn_list = vec![];
        match dir.desc().content().obj_list() {
            NDNObjectInfo::ObjList(list) => {
                for (path, node) in list.object_map() {
                    let path = path.trim_start_matches('/');
                    if path.starts_with("list/") {
                        let (sn_id, buf) = Self::load_sn_node(&id, &dir, &node)?;
                        match Device::clone_from_slice(&buf) {
                            Ok(device) => {
                                let real_id = device.desc().device_id();
                                if real_id != *sn_id {
                                    let msg = format!(
                                        "sn device id not matched with configed in dir! dir={}, config={}, real={}",
                                        id, sn_id, real_id,
                                    );
                                    error!("{}", msg);
                                    return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                                }

                                info!("got sn device from dir config: dir={}, sn={}", id, real_id);
                                sn_list.push((real_id, device));
                            }
                            Err(e) => {
                                let msg = format!(
                                    "invalid sn device object: dir={}, sn={}, {}",
                                    id, sn_id, e
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                            }
                        }
                    }
                }
            }
            NDNObjectInfo::Chunk(_) => {
                let msg = format!("dir chunk mode for sn config not support! id={}", id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
            }
        }

        Ok(sn_list)
    }

    pub fn load_sn_node<'a>(
        id: &ObjectId,
        dir: &'a Dir,
        node: &'a InnerNodeInfo,
    ) -> BuckyResult<(&'a ObjectId, &'a Vec<u8>)> {
        match node.node() {
            InnerNode::ObjId(sn_id) => {
                match dir.body() {
                    Some(body) => {
                        match body.content() {
                            DirBodyContent::ObjList(list) => {
                                match list.get(sn_id) {
                                    Some(value) => Ok((sn_id, value)),
                                    None => {
                                        let msg = format!("load sn item from dir body but not found! id={}, sn={}", id, sn_id);
                                        error!("{}", msg);
                                        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                                    }
                                }
                            }
                            _ => {
                                let msg = format!("dir body chunk content format for sn config not support! id={}", id);
                                error!("{}", msg);
                                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
                            }
                        }
                    }
                    None => {
                        let msg = format!("dir body content not exists! id={}", id);
                        error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
                    }
                }
            }
            _ => {
                let msg = format!(
                    "dir desc inner node format for sn config not support! id={}, node={:?}",
                    id, node
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }
}

use std::collections::HashMap;
use std::path::Path;

use super::bdt_util::load_device_objects_list;

pub struct SNDirGenerator;

impl SNDirGenerator {
    pub fn gen_from_dir(owner_id: &Option<ObjectId>, root: &Path) -> BuckyResult<Dir> {
        let list = load_device_objects_list(root);
        if list.is_empty() {
            let msg = format!("sn device folder is empty! dir={}", root.display());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        Self::gen_from_list(owner_id, list)
    }

    pub fn gen_from_list(
        owner_id: &Option<ObjectId>,
        list: Vec<(DeviceId, Device)>,
    ) -> BuckyResult<Dir> {
        let mut object_map = HashMap::new();
        let mut body_map = HashMap::new();
        for (id, sn) in list {
            let inner_node = InnerNodeInfo::new(
                Attributes::default(),
                InnerNode::ObjId(id.object_id().clone()),
            );

            let path = format!("list/{}", id);
            info!("will add sn item to dir: {}", path);
            if let Some(_prev) = object_map.insert(path, inner_node) {
                let msg = format!("sn device item already exists in desc! id={}", id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }

            let buf = sn.to_vec()?;
            if let Some(_prev) = body_map.insert(id.object_id().clone(), buf) {
                let msg = format!("sn device item already exists in body! id={}", id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }
        }

        let list = NDNObjectList {
            parent_chunk: None,
            object_map,
        };

        let attr = Attributes::new(0);
        let builder = Dir::new(attr, NDNObjectInfo::ObjList(list.clone()), body_map);
        let dir = builder
            .option_owner(owner_id.to_owned())
            .no_create_time()
            .update_time(bucky_time_now())
            .build();
        let dir_id = dir.desc().calculate_id();

        info!("gen sn dir complete! dir={}", dir_id);
        Ok(dir)
    }
}

#[cfg(test)]
mod test {
    use cyfs_base::*;

    use crate::*;

    #[test]
    fn test() {
        crate::init_log("test-sn-dir", None);

        let root = crate::get_cyfs_root_path_ref().join("etc");
        let dir = SNDirGenerator::gen_from_dir(&None, &root).unwrap();
        let object_raw = dir.to_vec().unwrap();
        let dir_id = dir.desc().calculate_id();
        let list = SNDirParser::parse(Some(&dir_id), &object_raw).unwrap();
        for item in list {
            info!("got sn item: {}", item.0);
        }
    }
}
