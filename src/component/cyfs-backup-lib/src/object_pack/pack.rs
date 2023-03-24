use cyfs_base::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPackFileInfo {
    pub name: String,
    pub hash: HashValue,
    pub file_len: u64,
    pub data_len: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum ObjectPackFormat {
    Zip,
}
