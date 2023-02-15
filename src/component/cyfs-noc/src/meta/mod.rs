mod meta;
mod sqlite;
mod access;
mod cache;

pub use meta::*;
pub(crate) use access::*;


use cyfs_base::BuckyResult;

use std::path::Path;
use std::sync::Arc;

pub(crate) fn create_meta(root: &Path) -> BuckyResult<meta::NamedObjectMetaRef> {
    let meta = sqlite::SqliteMetaStorage::new(root)?;
    let meta = Arc::new(Box::new(meta) as Box<dyn NamedObjectMeta>);

    let meta_with_cache = cache::NamedObjectMetaWithAccessCache::new(meta);
    let meta_with_cache = Arc::new(Box::new(meta_with_cache) as Box<dyn NamedObjectMeta>);

    Ok(meta_with_cache)
}
