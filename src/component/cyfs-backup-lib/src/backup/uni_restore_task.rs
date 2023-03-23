use crate::crypto::*;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct UniRestoreParams {
    pub id: String,
    pub cyfs_root: String,
    pub isolate: String,
    pub archive: PathBuf,
    pub password: Option<ProtectedPassword>,
}