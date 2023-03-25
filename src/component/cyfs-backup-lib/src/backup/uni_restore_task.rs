use crate::crypto::*;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniRestoreParams {
    pub id: String,
    pub cyfs_root: String,
    pub isolate: String,
    pub archive: PathBuf,
    pub password: Option<ProtectedPassword>,
}