use cyfs_base::*;

use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::str::FromStr;


// 目前revision的管理只是动态内容，重启后会清空

struct RevisionListImpl {
    // keep all dec list
    dec_list: HashSet<ObjectId>,

    // dec_root和global_root的映射
    dec_index: HashMap<ObjectId, ObjectId>,

    // global_root和u64的映射
    revision_list: HashMap<ObjectId, u64>,
}

impl RevisionListImpl {
    pub fn new() -> Self {
        Self {
            dec_list: HashSet::new(),
            dec_index: HashMap::new(),
            revision_list: HashMap::new(),
        }
    }

    pub fn is_dec_exists(&self, dec_id: &ObjectId) -> bool {
        self.dec_list.contains(dec_id)
    }

    pub fn insert_dec_root(
        &mut self,
        dec_id: &ObjectId,
        dec_root: ObjectId,
        global_root: ObjectId,
    ) {
        self.dec_list.insert(dec_id.to_owned());

        match self.dec_index.entry(dec_root.clone()) {
            Entry::Vacant(v) => {
                info!(
                    "dec root assoc with global root: dec={}, dec_root={}, global_root={}",
                    dec_id, dec_root, global_root
                );
                v.insert(global_root);
            }
            Entry::Occupied(o) => {
                // 只需要第一次关联，如果相同的dec_root提交，但中间的global_root被其它dec改了，会造成不一致
                info!("dec_root already exists! dec={}, dec_root={}, current_global_root={}, new_global_root={},"
                    , dec_id, dec_root, o.get(), global_root);
            }
        }
    }

    pub fn insert_revision(&mut self, revision: u64, global_root: ObjectId) {
        let _ret = self.revision_list.insert(global_root, revision);
        // assert!(ret.is_none());
    }

    pub fn get_root_revision(&self, global_root: &ObjectId) -> Option<u64> {
        self.revision_list.get(global_root).cloned()
    }

    pub fn get_dec_relation_root_info(&self, dec_root: &ObjectId) -> (ObjectId, u64) {
        let global_root = self.dec_index.get(dec_root).unwrap();
        let revision = self.revision_list.get(global_root).unwrap();

        (global_root.to_owned(), revision.to_owned())
    }
}

#[derive(Clone)]
pub(crate) struct RevisionList(Arc<RwLock<RevisionListImpl>>);

impl RevisionList {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(RevisionListImpl::new())))
    }

    pub fn is_dec_exists(&self, dec_id: &ObjectId) -> bool {
        self.0.read().unwrap().is_dec_exists(dec_id)
    }

    // dec至少commit一次后才会有映射关系
    pub fn insert_dec_root(&self, dec_id: &ObjectId, dec_root: ObjectId, global_root: ObjectId) {
        self.0
            .write()
            .unwrap()
            .insert_dec_root(dec_id, dec_root, global_root)
    }

    pub fn insert_revision(&self, revision: u64, global_root: ObjectId) {
        self.0
            .write()
            .unwrap()
            .insert_revision(revision, global_root)
    }

    pub fn get_root_revision(&self, global_root: &ObjectId) -> Option<u64> {
        self.0.read().unwrap().get_root_revision(global_root)
    }

    pub fn get_dec_relation_root_info(&self, dec_root: &ObjectId) -> (ObjectId, u64) {
        self.0.read().unwrap().get_dec_relation_root_info(dec_root)
    }

    // FIXME 启动时候，加载dec_root和global_root的关系，需要注意这个可能会随着global_root的增长而变动
    // 如果需要确切的映射关系，revision list需要持久化
    pub async fn update_dec_relation(&self, root: &ObjectMapRootManager) -> BuckyResult<()> {
        let op_env = root.create_op_env(None).await?;
        let global_root = op_env.root();
        let dec_list = op_env.list("/").await?;

        let mut this = self.0.write().unwrap();
        for item in dec_list.list {
            match item {
                ObjectMapContentItem::Map((dec_id, dec_root)) => {
                    match ObjectId::from_str(&dec_id) {
                        Ok(dec_id) => {
                            this.insert_dec_root(&dec_id, dec_root, global_root.clone());
                        }
                        Err(e) => {
                            error!("load dec_root key from global_root but invalid: key={}, {}", dec_id, e);
                        }
                    }
                }
                _ => {
                    unreachable!();
                }
            }
        }

        Ok(())
    }
}
