use cyfs_lib::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStatePathConfigList {
    list: Vec<GlobalStatePathConfigItem>,
}

impl Default for GlobalStatePathConfigList {
    fn default() -> Self {
        Self { list: vec![] }
    }
}

impl GlobalStatePathConfigList {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    pub fn sort(&mut self) {
        self.list.sort_by(|left, right| {
            GlobalStatePathHelper::compare_path(&right.path, &left.path).unwrap()
        })
    }

    // return true if any changed
    pub fn add(&mut self, mut item: GlobalStatePathConfigItem) -> bool {
        item.try_fix_path();

        if let Ok(i) = self
            .list
            .binary_search_by(|v| GlobalStatePathHelper::compare_path(&v.path, &item.path).unwrap())
        {
            if item == self.list[i] {
                return false;
            }

            info!("rconfig replace item: {:?} -> {:?}", self.list[i], item);
            self.list[i] = item;
        } else {
            info!("new rconfig item: {:?}", item);
            self.list.push(item);
            self.sort();
        }

        true
    }

    pub fn remove(
        &mut self,
        mut item: GlobalStatePathConfigItem,
    ) -> Option<GlobalStatePathConfigItem> {
        item.try_fix_path();

        if let Ok(i) = self
            .list
            .binary_search_by(|v| GlobalStatePathHelper::compare_path(&v.path, &item.path).unwrap())
        {
            let item = self.list.remove(i);
            info!("rconfig remove item: {:?}", item);
            Some(item)
        } else {
            info!("rconfig remove item but not found: {:?}", item);
            None
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

    pub fn query(&self, path: &str) -> Option<&GlobalStatePathConfigItem> {
        let path = GlobalStatePathHelper::fix_path(path);

        // 路径越长，在列表中的位置越靠前
        let mut current_len = usize::MAX;
        for item in &self.list {

            // FIXME check codes for debug
            assert!(item.path.len() <= current_len);
            current_len = item.path.len();

            if path.len() > item.path.len() {
                continue;
            }

            if path.starts_with(item.path.as_str()) {
                return Some(&item);
            }
        }

        None
    }
}
