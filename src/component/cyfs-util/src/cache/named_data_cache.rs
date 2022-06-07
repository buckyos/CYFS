use cyfs_base::*;

use async_trait::async_trait;
use int_enum::IntEnum;

#[derive(Debug, Clone)]
pub struct FileDirRef {
    pub dir_id: DirId,
    pub inner_path: String,
}

#[derive(Debug)]
pub struct FileCacheData {
    pub hash: String,

    pub file_id: FileId,

    pub length: u64,

    pub flags: u32,

    pub owner: Option<ObjectId>,

    // 可选，关联的quickhash
    pub quick_hash: Option<Vec<String>>,

    // 可选，关联的dirs
    pub dirs: Option<Vec<FileDirRef>>,
}

pub struct InsertFileRequest {
    pub file_id: FileId,

    // 需要插入的file对象
    pub file: File,

    pub flags: u32,

    // 关联的quickhash
    pub quick_hash: Option<Vec<String>>,

    // 关联的dirs信息
    pub dirs: Option<Vec<FileDirRef>>,
}

pub struct RemoveFileRequest {
    pub file_id: FileId,
}

// quickhash相关操作
pub struct FileAddQuickhashRequest {
    pub hash: String,
    
    // TODO 是否支持以file_id为键值来更新？
    // pub file_id: FileId,

    pub quick_hash: Vec<String>,
}

pub struct FileUpdateQuickhashRequest {
    pub hash: String,
    
    // TODO 是否支持以file_id为键值来更新？
    // pub file_id: FileId,

    pub add_list: Vec<String>,
    pub remove_list: Vec<String>,
}

// GetFileByHashRequest的标志位，表示是不是要获取对应的信息
pub const NDC_FILE_REQUEST_FLAG_QUICK_HASN: u32 = 0x01 << 1;
pub const NDC_FILE_REQUEST_FLAG_REF_DIRS: u32 = 0x01 << 2;

pub struct GetFileByHashRequest {
    pub hash: String,

    pub flags: u32,
}

pub struct GetFileByFileIdRequest {
    pub file_id: FileId,

    pub flags: u32,
}

pub struct GetFileByQuickHashRequest {
    pub quick_hash: String,
    pub length: u64,
    pub flags: u32,
}

pub struct GetFileByChunkRequest {
    pub chunk_id: ChunkId,
    pub flags: u32,
}

pub struct GetDirByFileRequest {
    pub file_id: FileId,
    pub flags: u32,
}

// chunk的状态
// chunk和object的关系
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, IntEnum)]
pub enum ChunkObjectRelation {
    Unknown = 0,
    FileBody = 1,
    DirMeta = 2,
}

impl Into<u8> for ChunkObjectRelation {
    fn into(self) -> u8 {
        unsafe { std::mem::transmute(self as u8) }
    }
}

impl From<u8> for ChunkObjectRelation {
    fn from(code: u8) -> Self {
        match ChunkObjectRelation::from_int(code) {
            Ok(code) => code,
            Err(e) => {
                error!("unknown ChunkObjectRelation code: {} {}", code, e);
                ChunkObjectRelation::Unknown
            }
        }
    }
}

// chunk关联的对象
#[derive(Debug, Clone)]
pub struct ChunkObjectRef {
    pub object_id: ObjectId,
    pub relation: ChunkObjectRelation,
}

//#[derive(Clone)]
pub struct InsertChunkRequest {
    pub chunk_id: ChunkId,

    pub state: ChunkState,

    // 有关系的对象
    pub ref_objects: Option<Vec<ChunkObjectRef>>,

    // 所属的trans_session列表
    pub trans_sessions: Option<Vec<String>>,

    pub flags: u32,
}

pub struct RemoveChunkRequest {
    pub chunk_id: ChunkId,
}

// GetChunkRequest的额外查询信息
pub const NDC_CHUNK_REQUEST_FLAG_TRANS_SESSIONS: u32 = 0x01 << 1;
pub const NDC_CHUNK_REQUEST_FLAG_REF_OBJECTS: u32 = 0x01 << 2;

pub struct GetChunkRequest {
    pub chunk_id: ChunkId,
    pub flags: u32,
}

pub struct ExistsChunkRequest {
    pub chunk_list: Vec<ChunkId>,
    pub states: Vec<ChunkState>,
}

pub struct ChunkCacheData {
    pub chunk_id: ChunkId,

    pub state: ChunkState,
    pub flags: u32,
    
    pub insert_time: u64,
    pub update_time: u64,
    pub last_access_time: u64,

    pub trans_sessions: Option<Vec<String>>,
    pub ref_objects: Option<Vec<ChunkObjectRef>>,
}


pub struct UpdateChunkStateRequest {
    pub chunk_id: ChunkId,

    // 如果指定了当前状态，那么只有在匹配情况下才更新到目标状态
    pub current_state: Option<ChunkState>,
    
    pub state: ChunkState,
}

pub struct UpdateChunkTransSessionRequest {
    pub chunk_id: ChunkId,

    pub add_list: Vec<String>,
    pub remove_list: Vec<String>,
}

pub struct UpdateChunkRefsRequest {
    pub chunk_id: ChunkId,

    pub add_list: Vec<ChunkObjectRef>,
    pub remove_list: Vec<ChunkObjectRef>,
}

pub struct GetChunkTransSessionsRequest {
    pub chunk_id: ChunkId,
}

pub struct GetChunkTransSessionsResponse {
    pub chunk_id: ChunkId,
    pub trans_sessions: Option<Vec<String>>,
}

pub struct GetChunkRefObjectsRequest {
    pub chunk_id: ChunkId,
    pub relation: Option<ChunkObjectRelation>,
}

pub struct GetChunkRefObjectsResponse {
    pub chunk_id: ChunkId,
    pub ref_objects: Option<ChunkObjectRef>,
}

#[async_trait]
pub trait NamedDataCache: Sync + Send + 'static {

    fn clone(&self) -> Box<dyn NamedDataCache>;

    // file相关接口
    async fn insert_file(&self, req: &InsertFileRequest) -> BuckyResult<()>;
    async fn remove_file(&self, req: &RemoveFileRequest) -> BuckyResult<usize>;

    async fn file_update_quick_hash(&self, req: &FileUpdateQuickhashRequest) -> BuckyResult<()>;

    async fn get_file_by_hash(&self, req: &GetFileByHashRequest) -> BuckyResult<Option<FileCacheData>>;
    async fn get_file_by_file_id(&self, req: &GetFileByFileIdRequest) -> BuckyResult<Option<FileCacheData>>;
    async fn get_files_by_quick_hash(&self, req: &GetFileByQuickHashRequest) -> BuckyResult<Vec<FileCacheData>>;
    async fn get_files_by_chunk(&self, req: &GetFileByChunkRequest) -> BuckyResult<Vec<FileCacheData>>;
    async fn get_dirs_by_file(&self, req: &GetDirByFileRequest) -> BuckyResult<Vec<FileDirRef>>;

    // chunk相关接口
    async fn insert_chunk(&self, req: &InsertChunkRequest) -> BuckyResult<()>;
    async fn remove_chunk(&self, req: &RemoveChunkRequest) -> BuckyResult<usize>;

    async fn update_chunk_state(&self, req: &UpdateChunkStateRequest) -> BuckyResult<ChunkState>;
    //async fn update_chunk_trans_session(&self, req: &UpdateChunkTransSessionRequest) -> BuckyResult<()>;
    async fn update_chunk_ref_objects(&self, req: &UpdateChunkRefsRequest) -> BuckyResult<()>;

    async fn exists_chunks(&self, req: &ExistsChunkRequest) -> BuckyResult<Vec<bool>>;

    async fn get_chunk(&self, req: &GetChunkRequest) -> BuckyResult<Option<ChunkCacheData>>;
    async fn get_chunks(&self, req: &Vec<GetChunkRequest>) -> BuckyResult<Vec<Option<ChunkCacheData>>>;
    //async fn get_chunk_trans_sessions(&self, req: &GetChunkTransSessionsRequest) -> BuckyResult<GetChunkTransSessionsResponse>;
    async fn get_chunk_ref_objects(&self, req: &GetChunkRefObjectsRequest) -> BuckyResult<Vec<ChunkObjectRef>>;
}