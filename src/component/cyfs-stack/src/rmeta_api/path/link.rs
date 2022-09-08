use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStatePathLinkList {
    list: Vec<GlobalStatePathLinkItem>,
}

impl Default for GlobalStatePathLinkList {
    fn default() -> Self {
        Self { list: vec![] }
    }
}

const GLOBAL_STATE_PATH_LINK_MAX_DEPTH: u8 = 32;

impl GlobalStatePathLinkList {
    pub fn sort(&mut self) {
        self.list
            .sort_by(|left, right| right.source.cmp(&left.source))
    }

    /*
    /a -> /a/
    /a/b -> /a/b/
    / -> Invalid
    */
    fn fix_path(path: impl Into<String> + AsRef<str>) -> BuckyResult<String> {
        let path = path.as_ref().trim();
        if path == "/" {
            let msg = format!("UnSupport objectmap path link source! path={}", path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        // 确保末尾以/结束
        let ret = match path.ends_with("/") {
            true => path.into(),
            false => format!("{}/", path.as_ref() as &str),
        };

        Ok(ret)
    }

    pub fn add(
        &mut self,
        source: impl Into<String> + AsRef<str>,
        target: impl Into<String> + AsRef<str>,
    ) -> BuckyResult<bool> {
        let source = Self::fix_path(source)?;
        let target = Self::fix_path(target)?;

        for item in &self.list {
            if item.source == source {
                if item.target == target {
                    warn!("path link already exists! {} -> {}", source, target);
                    return Ok(false);
                } else {
                    let msg = format!("path link already exists but target is different! source={}, current={}, new={}", 
                    source, target, item.target);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
                }
            }
        }

        info!("new path link: {} -> {}", source, target);
        let item = GlobalStatePathLinkItem { source, target };
        self.list.push(item);
        self.sort();

        Ok(true)
    }

    pub fn remove(&mut self, source: &str) -> BuckyResult<Option<GlobalStatePathLinkItem>> {
        let source = Self::fix_path(source)?;
        match self.list.binary_search_by(|item| item.source.cmp(&source)) {
            Ok(index) => {
                let item = self.list.remove(index);
                info!("remove path link: {} -> {}", source, item.target);
                Ok(Some(item))
            }
            Err(_) => {
                let msg = format!("remove path link but not found! {}", source);
                warn!("{}", msg);
                Ok(None)
            }
        }
    }

    pub fn clear(&mut self) -> usize {
        if self.list.is_empty() {
            return 0;
        }

        let count = self.list.len();
        self.list.clear();
        count
    }
    
    pub fn get(&self) -> Vec<GlobalStatePathLinkItem> {
        self.list.clone()
    }

    fn translate_once(&self, source: &str) -> Option<String> {
        assert!(source.ends_with('/'));

        for item in &self.list {
            if source.starts_with(&item.source) {
                let dest = format!("{}{}", item.target, &source[item.source.len()..]);
                return Some(dest);
            }
            if source.len() < item.source.len() {
                break;
            }
        }

        None
    }

    pub fn resolve(&self, source: &str) -> BuckyResult<Option<String>> {
        assert!(source.ends_with('/'));

        let mut ret = Cow::Borrowed(source);
        let mut depth = 0;
        loop {
            match self.translate_once(&ret) {
                Some(dest) => {
                    info!("resolve path link: {} -> {}", ret, dest);
                    ret = Cow::Owned(dest);
                    depth += 1;
                    if depth >= GLOBAL_STATE_PATH_LINK_MAX_DEPTH {
                        let msg = format!(
                            "resolve path link extend max depth limit! source={}",
                            source
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
                    }
                }
                None => {
                    break;
                }
            }
        }

        if ret == source {
            Ok(None)
        } else {
            Ok(Some(ret.to_string()))
        }
    }
}

#[cfg(test)]
mod test_path_link {
    use super::*;

    #[test]
    fn tes_sort() {
        cyfs_base::init_simple_log("test_path_link", None);
        let mut links = GlobalStatePathLinkList::default();

        links.add("/x", "/a/c/e").unwrap();
        links.add("/y", "/a/c/e/f").unwrap();
        links.add("/", "/a").unwrap_err();

        let ret=  links.resolve("/x/b/c/").unwrap();
        println!("{:?}", ret);
    }
}
