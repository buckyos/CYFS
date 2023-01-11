use std::sync::atomic::{AtomicU8, Ordering};

use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;
use sha2::Digest;

// TODO: 后面再封装这个对象
#[derive(Clone, RawEncode, RawDecode)]
pub struct GroupRPathStatus {}

#[cfg(test)]
mod test {}
