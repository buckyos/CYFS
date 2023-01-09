//// NON request flags
/// 
// router get_object触发本地object刷新操作，如果存在缓存
pub const CYFS_ROUTER_REQUEST_FLAG_FLUSH: u32 = 0x01 << 0;

// delete操作是否返回原值，默认不返回
pub const CYFS_REQUEST_FLAG_DELETE_WITH_QUERY: u32 = 0x01 << 1;

// get_object，列举当前dir/inner_path下的内容
pub const CYFS_REQUEST_FLAG_LIST_DIR: u32 = 0x01 << 2;

//// NDN request flags
///
// get_data/trans_task，target object is file/dir, 跨device请求直接使用chunk级别的acl，不再使用所属的file/dir
pub const CYFS_REQUEST_FLAG_CHUNK_LEVEL_ACL: u32 = 0x01 << 3;