use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalStatePathStorageState {
    Concrete = 0,
    Virtual = 1,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStatePathConfig {
    path: String,

    // 要求存储状态，如为virtual则重建时会跳过。
    storage_state: Option<u8>,

    // 重建深度.0表示无引用深度，1表示会重建其引用的1层对象。不配置则根据对象的Selector确定初始重建深度。对大文件不自动重建，需要手动将depth设置为1.
    depth: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStatePathConfigList {
    list: Vec<GlobalStatePathConfig>,
}

impl Default for GlobalStatePathConfigList {
    fn default() -> Self {
        Self { list: vec![] }
    }
}

impl GlobalStatePathConfigList {
    pub fn new() -> Self {
        Self {
            list: vec![],
        }
    }
    
    pub fn sort(&mut self) {
        self.list.sort_by(|left, right| right.path.cmp(&left.path))
    }
}