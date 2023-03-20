use super::cache::*;
use super::iterator::IntoObjectMapContentItem;
use super::object_map::*;
use crate::*;

use std::collections::VecDeque;

// 对多级ObjectMap进行遍历的迭代器
// 树状结构的迭代一般分为广度优先和深度优先，但由于我们支持step>1的优化遍历，所以广度优先会让性能更高些
// 深度优先遍历话内部节点要使用step(1)来模拟，性能要差些

pub struct ObjectMapPathContentItem {
    pub path: String,
    pub value: ObjectMapContentItem,
    pub content_type: ObjectMapSimpleContentType,
}

impl std::fmt::Debug for ObjectMapPathContentItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} item: {}={}",
            self.content_type, self.path, self.value
        )
    }
}

#[derive(Debug)]
pub struct ObjectMapPathContentList {
    pub list: Vec<ObjectMapPathContentItem>,
}

impl std::fmt::Display for ObjectMapPathContentList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "path content list: {:?}", self.list)
    }
}

impl ObjectMapPathContentList {
    pub fn new(capacity: usize) -> Self {
        Self {
            list: Vec::with_capacity(capacity),
        }
    }
}

struct PathIteratorSeg {
    path: String,
    it: ObjectMapBindIterator,
}

#[derive(Clone, Debug)]
pub struct ObjectMapPathIteratorOption {
    pub leaf_object: bool,
    pub mid_object: bool,
}

impl Default for ObjectMapPathIteratorOption {
    fn default() -> Self {
        Self {
            leaf_object: true,
            mid_object: false,
        }
    }
}

impl ObjectMapPathIteratorOption {
    pub fn new(leaf_object: bool, mid_object: bool) -> Self {
        Self {
            leaf_object,
            mid_object,
        }
    }
}

pub struct ObjectMapPathIterator {
    cache: ObjectMapOpEnvCacheRef,

    stack: VecDeque<PathIteratorSeg>,

    opt: ObjectMapPathIteratorOption,
}

impl ObjectMapPathIterator {
    pub async fn new(target: ObjectMapRef, cache: ObjectMapOpEnvCacheRef, opt: ObjectMapPathIteratorOption) -> Self {
        let mut ret = Self {
            cache,
            stack: VecDeque::new(),
            opt,
        };

        let seg = PathIteratorSeg {
            path: "/".to_owned(),
            it: ObjectMapBindIterator::new_with_target(target, ret.cache.clone()).await,
        };
        ret.stack.push_front(seg);

        ret
    }

    fn join_path(parent: &str, key: &str) -> String {
        if parent.ends_with("/") {
            format!("{}{}", parent, key)
        } else {
            format!("{}/{}", parent, key)
        }
    }

    pub fn is_end(&self) -> bool {
        self.stack.is_empty()
    }

    // 广度优先遍历
    pub async fn next(&mut self, step: usize) -> BuckyResult<ObjectMapPathContentList> {
        assert!(step > 0);

        let mut result = ObjectMapPathContentList::new(step);
        let mut remaining = step;
        loop {
            if self.stack.is_empty() {
                break;
            }

            let ret;
            let path;
            {
                let current = self.stack.back_mut().unwrap();
                path = current.path.clone();
                ret = current.it.next(remaining).await?;
            }

            if ret.list.len() < remaining {
                self.stack.pop_back();
            }

            for item in ret.list {
                match item {
                    ObjectMapContentItem::Map((key, value)) => {
                        match value.obj_type_code() {
                            ObjectTypeCode::ObjectMap => {
                                let next_path = Self::join_path(&path, &key);

                                let mut got = self.opt.mid_object;
                                if let Err(e) = self.pending_objectmap(&next_path, &value).await {
                                    warn!("iterator over objectmap error, now will treat as normal object! path={}, key={}, value={}, {}", path, key, value, e);

                                    // FIXME 如果其中一个objectmap遍历失败了，是终止遍历，还是把这个id当成一个普通对象来处理？
                                    got = self.opt.leaf_object;
                                }

                                if got {
                                    let item = ObjectMapPathContentItem {
                                        path: path.clone(),
                                        value: value.into_content(Some(&key)),
                                        content_type: ObjectMapSimpleContentType::Map,
                                    };
                                    result.list.push(item);
                                    remaining -= 1;
                                }
                            }
                            _ => {
                                if self.opt.leaf_object {
                                    let item = ObjectMapPathContentItem {
                                        path: path.clone(),
                                        value: value.into_content(Some(&key)),
                                        content_type: ObjectMapSimpleContentType::Map,
                                    };
    
                                    result.list.push(item);
                                    remaining -= 1;
                                }
                            }
                        }
                    }
                    ObjectMapContentItem::DiffMap((key, value)) => {
                        match &value.diff {
                            Some(diff) => {
                                let next_path = Self::join_path(&path, &key);

                                let mut got = self.opt.mid_object;
                                if let Err(e) = self.pending_objectmap(&next_path, &diff).await {
                                    warn!("iterator over objectmap diff error, now will treat as normal diff object! path={}, key={}, value={}, {}", path, key, value, e);

                                    // FIXME 如果其中一个objectmap遍历失败了，是终止遍历，还是把这个id当成一个普通对象来处理？
                                    got = self.opt.leaf_object;
                                }

                                if got {
                                    let item = ObjectMapPathContentItem {
                                        path: path.clone(),
                                        value: value.into_content(Some(&key)),
                                        content_type: ObjectMapSimpleContentType::DiffMap,
                                    };
                                    result.list.push(item);
                                    remaining -= 1;
                                }
                            }
                            None => {
                                if self.opt.leaf_object {
                                    let item = ObjectMapPathContentItem {
                                        path: path.clone(),
                                        value: value.into_content(Some(&key)),
                                        content_type: ObjectMapSimpleContentType::DiffMap,
                                    };
    
                                    result.list.push(item);
                                    remaining -= 1;
                                }
                            }
                        }
                    }
                    ObjectMapContentItem::Set(value) => {
                        match value.obj_type_code() {
                            ObjectTypeCode::ObjectMap => {

                                let mut got = self.opt.mid_object;
                                if let Err(e) = self.pending_objectmap(&path, &value).await
                                {
                                    warn!("iterator over objectmap error, now will treat as normal object! path={}, value={}, {}", path, value, e);

                                    // FIXME 如果其中一个objectmap遍历失败了，是终止遍历，还是把这个id当成一个叶子对象来处理？
                                    got = self.opt.leaf_object;
                                }

                                if got {
                                    let item = ObjectMapPathContentItem {
                                        path: path.clone(),
                                        value: value.into_content(None),
                                        content_type: ObjectMapSimpleContentType::Set,
                                    };

                                    result.list.push(item);
                                    remaining -= 1;
                                }
                            }
                            _ => {
                                if self.opt.leaf_object {
                                    let item = ObjectMapPathContentItem {
                                        path: path.clone(),
                                        value: value.into_content(None),
                                        content_type: ObjectMapSimpleContentType::Set,
                                    };
    
                                    result.list.push(item);
                                    remaining -= 1;
                                }
                            }
                        }
                    }
                    ObjectMapContentItem::DiffSet(value) => {
                        if self.opt.leaf_object {
                            let item = ObjectMapPathContentItem {
                                path: path.clone(),
                                value: value.into_content(None),
                                content_type: ObjectMapSimpleContentType::DiffSet,
                            };
    
                            result.list.push(item);
                            remaining -= 1;
                        }
                    }
                }
            }

            if remaining == 0 {
                assert_eq!(result.list.len(), step);
                break;
            }
        }

        Ok(result)
    }

    async fn pending_objectmap(&mut self, path: &str, value: &ObjectId) -> BuckyResult<()> {

        // 尝试从cache加载目标
        let target = self.cache.get_object_map(value).await?;
        if target.is_none() {
            let msg = format!(
                "iterator with sub objectmap but not found! path={}, objmap={}",
                path, value
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let target = target.unwrap();
        let it = ObjectMapBindIterator::new_with_target(target, self.cache.clone()).await;

        let pending_seg = PathIteratorSeg {
            path: path.to_owned(),
            it,
        };

        self.stack.push_front(pending_seg);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::cache::*;
    use super::super::path::*;
    use super::*;

    use std::str::FromStr;

    async fn gen_path(path: &ObjectMapPath) -> ObjectId {
        let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
        let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();
        info!(
            "typecode: {:?}, {:?}",
            x1_value.obj_type_code(),
            x1_value2.obj_type_code()
        );

        for i in 0..100 {
            let key = format!("/a/b/{}", i);
            path.insert_with_path(&key, &x1_value).await.unwrap();
        }

        for i in 0..100 {
            let key = format!("/a/c/{}", i);
            path.insert_with_path(&key, &x1_value).await.unwrap();
        }


        for i in 0..10 {
            let key = format!("/a/b_{}", i);
            path.insert_with_path(&key, &x1_value2).await.unwrap();
        }

        for i in 0..10 {
            let key = format!("/a_{}", i);
            path.insert_with_path(&key, &x1_value2).await.unwrap();
        }

        path.root()
    }

    async fn test_path_iterator() {
        let noc = ObjectMapMemoryNOCCache::new();
        let root_cache = ObjectMapRootMemoryCache::new_default_ref(None, noc);
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        // 创建一个空的objectmap作为root
        let owner = ObjectId::default();
        let root = ObjectMap::new(
            ObjectMapSimpleContentType::Map,
            Some(owner.clone()),
            Some(owner.clone()),
        )
        .no_create_time()
        .build();
        let root_id = root.flush_id();
        cache.put_object_map(&root_id, root, None).unwrap();
        info!("new root: {}", root_id);

        let path = ObjectMapPath::new(root_id.clone(), cache.clone(), true);
        let path_root = gen_path(&path).await;
        info!("generated path root: {}", path_root);

        let root = cache.get_object_map(&path_root).await.unwrap();
        let mut it = ObjectMapPathIterator::new(root.unwrap(), cache, ObjectMapPathIteratorOption::default()).await;
        while !it.is_end() {
            let list = it.next(1).await.unwrap();
            info!("list: {} {:?}", 1, list.list);
        }
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map-path-iterator", Some("debug"));
        async_std::task::block_on(async move {
            test_path_iterator().await;
        });
    }
}
