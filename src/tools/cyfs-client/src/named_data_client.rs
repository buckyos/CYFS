use crate::ffs_client_util::{generate_file_desc, write_id_to_file, generate_dir_desc_2};
use crate::meta_helper;
use async_std::io::{copy as async_copy};
use async_std::io::Write as AsyncWrite;
use async_std::prelude::*;
use cyfs_chunk_client::{ChunkClient, ChunkClientContext, ChunkSourceContext};
use http_types::{Method, Request};
use rand::{Rng, RngCore};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::io::{Read};
use std::marker::Unpin;
use std::option::Option;
use std::path::{Path, PathBuf};
use std::result::Result::Ok;
use url::Url;

use async_trait::async_trait;
use async_std::sync::Mutex;
use log::*;
use std::time::{Duration, Instant};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use cyfs_base::*;
use cyfs_base_meta::*;
use std::str::FromStr;
use std::net::Shutdown;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use cyfs_bdt::stream_pool::{PooledStream, StreamPool, StreamPoolConfig};
use once_cell::sync::OnceCell;
use cyfs_bdt::{StackGuard, DeviceCache};

#[derive(Clone)]
struct BdtDeviceCache(Arc<BdtDeviceCacheImpl>);

struct BdtDeviceCacheImpl {
    cache: RwLock<HashMap<DeviceId, Device>>,
    meta_client: Arc<MetaClient>
}

impl BdtDeviceCache {
    fn new(meta_client: Arc<MetaClient>) -> Self {
        return Self(Arc::new(BdtDeviceCacheImpl{
            cache: RwLock::new(HashMap::new()),
            meta_client: meta_client.clone()
        }))
    }
}

#[async_trait]
impl DeviceCache for BdtDeviceCache {
    // 添加一个device并保存
    async fn add(&self, device_id: &DeviceId, device: Device) {
        self.0.cache.write().unwrap().insert(device_id.clone(), device);
    }

    // 直接在本地数据查询
    async fn get(&self, device_id: &DeviceId) -> Option<Device> {
        info!("bdt cache get id {}", device_id);
        self.search(device_id).await.ok()
    }

    // 本地查询，查询不到则发起查找操作
    async fn search(&self, device_id: &DeviceId) -> BuckyResult<Device> {
        info!("bdt cache search id {}", device_id);
        if let Some(device) = self.0.cache.read().unwrap().get(device_id) {
            return Ok(device.clone())
        }

        match self.0.meta_client.get_desc(device_id.object_id()).await? {
            SavedMetaObject::Device(device) => {
                self.0.cache.write().unwrap().insert(device_id.clone(), device.clone());
                Ok(device)
            }
            _ => {
                Err(BuckyError::from(BuckyErrorCode::NotMatch))
            }
        }
    }

    // 校验device的owner签名是否有效
    async fn verfiy_owner(&self, _device_id: &DeviceId, _device: Option<&Device>) -> BuckyResult<()> {
        Ok(())
    }

    // 有权对象的body签名自校验
    async fn verfiy_own_signs(&self, _object_id: &ObjectId, _object: &Arc<AnyNamedObject>) -> BuckyResult<()> {
        Ok(())
    }

    fn clone_cache(&self) -> Box<dyn DeviceCache> {
        Box::new(self.clone())
    }
}

pub struct NamedCacheClient {
    // 需要一个BDT栈，init时初始化
    stream_pool: OnceCell<StreamPool>,
    //desc: Device,
    //secret: PrivateKey,
    meta_client: Arc<MetaClient>,
    init_ret: Mutex<bool>,
    object_cache: RwLock<HashMap<ObjectId, StandardObject>>,
    device_cache: BdtDeviceCache,
    stack: OnceCell<StackGuard>,
    config: NamedCacheClientConfig
}

#[derive(PartialEq)]
pub enum ConnStrategy {
    BdtOnly,
    TcpFirst,
    TcpOnly
}

pub struct NamedCacheClientConfig {
    pub desc: Option<(Device, PrivateKey)>,
    pub meta_target: MetaMinerTarget,
    pub sn_list: Option<Vec<Device>>,
    pub area: Option<Area>,
    pub retry_times: u8,
    pub timeout: Duration,
    pub conn_strategy: ConnStrategy,
    pub tcp_chunk_manager_port: u16,
    pub tcp_file_manager_port: u16,
}

impl Default for NamedCacheClientConfig {
    fn default() -> Self {
        Self {
            desc: None,
            meta_target: MetaMinerTarget::default(),
            sn_list: None,
            area: None,
            retry_times: 3,
            timeout: Duration::from_secs(3*60),
            conn_strategy: ConnStrategy::BdtOnly,
            tcp_chunk_manager_port: CHUNK_MANAGER_PORT,
            tcp_file_manager_port: FILE_MANAGER_PORT
        }
    }
}

impl NamedCacheClient {
    pub fn new(config: NamedCacheClientConfig) -> NamedCacheClient {
        let client = Arc::new(MetaClient::new_target(config.meta_target.clone()).with_timeout(std::time::Duration::from_secs(60 * 2)));
        let device_cache = BdtDeviceCache::new(client.clone());
        NamedCacheClient {
            stream_pool: OnceCell::new(),
            //desc,
            //secret,
            meta_client: client,
            init_ret: Mutex::new(false),
            object_cache: RwLock::new(HashMap::new()),
            device_cache,
            stack: OnceCell::new(),
            config
        }
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
        // 避免并行init
        let mut ret = self.init_ret.lock().await;
        if *ret {
            return Ok(());
        }

        let (mut desc, secret) = if let Some((desc, secret)) = self.config.desc.clone() {
            (desc, secret)
        } else {
            info!("no input peerdesc, create random one.");
            let secret = PrivateKey::generate_rsa(1024)?;
            let public = secret.public();
            let area = self.config.area.clone().unwrap_or(Area::default());
            let mut uni = [0 as u8; 16];
            rand::thread_rng().fill_bytes(&mut uni);
            let uni_id = UniqueId::create(&uni);
            let desc = Device::new(None, uni_id, vec![], vec![], vec![], public, area, DeviceCategory::PC).build();
            (desc, secret)
        };

        info!("current device_id: {}", desc.desc().calculate_id());

        let endpoints = desc.body_mut().as_mut().unwrap().content_mut().mut_endpoints();
        if endpoints.len() == 0 {
            // 取随机端口号
            let port = rand::thread_rng().gen_range(30000, 50000) as u16;
            for ip in cyfs_util::get_all_ips().unwrap() {
                if ip.is_ipv4() {
                    endpoints.push(Endpoint::from((Protocol::Tcp, ip, port)));
                    endpoints.push(Endpoint::from((Protocol::Udp, ip, port)));
                }
            }
        }

        let mut params = cyfs_bdt::StackOpenParams::new("cyfs-client");
        if let Some(sn_list) = &self.config.sn_list {
            info!("named data client use param`s sn list {:?}", sn_list.iter().map(|device|{
                device.desc().calculate_id()
            }).collect::<Vec<ObjectId>>());
        }
        params.known_sn = self.config.sn_list.clone();
        params.outer_cache = Some(Box::new(self.device_cache.clone()));

        let stack = cyfs_bdt::Stack::open(desc, secret, params).await?;

        let pool = StreamPool::new(stack.deref().clone(), 80, StreamPoolConfig {
            capacity: 10,
            backlog: 10,
            atomic_interval: Duration::from_secs(5),
            timeout: Duration::from_secs(60)
        })?;

        info!("bdt stack created");
        let _ = self.stream_pool.set(pool);
        let _ = self.stack.set(stack);

        *ret = true;
        Ok(())
    }

    // 只支持cyfs链接
    // 只支持将文件夹内容存到指定path，不支持写到一个writer
    pub async fn get_by_url(&self, url: &str, dest: &Path) -> BuckyResult<()> {
        let (owner, id, inner) = self.extract_cyfs_url(url).await?;
        match id.obj_type_code() {
            ObjectTypeCode::File => {
                let mut dest_file = async_std::fs::File::create(dest).await?;
                let _desc = self.get_file_by_id_obj(&id, owner, &mut dest_file).await?;
                dest_file.flush().await?;
                Ok(())
            },
            ObjectTypeCode::Dir => {
                let inner = if inner.len()>0{Some(inner.as_str())}else{None};
                let _desc = self.get_dir_by_obj(&id, owner, inner, dest).await?;
                Ok(())
            },
            _ => {
                Err(BuckyError::from(BuckyErrorCode::NotSupport))
            }
        }
    }

    pub async fn reset_sn_list(&self, sn_list: Vec<Device>) -> BuckyResult<()> {
        if let Some(stack) = self.stack.get() {
            info!("named data client reset sn list {:?}", sn_list.iter().map(|device|{
                device.desc().calculate_id()
            }).collect::<Vec<ObjectId>>());
            stack.reset_sn_list(sn_list).wait_online().await?;
            Ok(())
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotInit))
        }
    }

    async fn get_id_from_str(&self, id_str: &str) -> BuckyResult<ObjectId> {
        match ObjectId::from_str(id_str) {
            Ok(id) => Ok(id),
            Err(_code) => {
                // TODO: str不是id，可能是name. 尝试从mata chain查询
                match self.meta_client.get_name(id_str).await? {
                    None => Err(BuckyError::from(BuckyErrorCode::NotFound)),
                    Some((info, state)) => match state {
                        NameState::Normal | NameState::Lock => {
                            return match info.record.link {
                                NameLink::ObjectLink(link) => Ok(link),
                                _ => Err(BuckyError::from(BuckyErrorCode::NotFound)),
                            }
                        }
                        _ => Err(BuckyError::from(BuckyErrorCode::NotFound)),
                    },
                }
            }
        }
    }

    async fn get_bdt_stream(&self, remote: &DeviceId) -> BuckyResult<PooledStream> {
        debug!("get bdt connection to {}:80", remote);
        self.stream_pool.get().unwrap().connect(remote).await
    }

    async fn get_chunk<W: ?Sized>(
        &self,
        chunk_id: &ChunkId,
        owner: &DeviceId,
        writer: &mut W,
    ) -> BuckyResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        // let peer_id = self.desc.get().unwrap().desc().device_id();
        let peer_id = self.stack.get().unwrap().local_device_id();
        let price = 0i64; // 本机获取，直接返回

        debug!("will get chunk by id {}", &chunk_id);

        //1. 从chunkManager取Chunk
        info!("try get chunk {} from local", &chunk_id);

        let chunk_get_data_req = cyfs_chunk::ChunkGetReq::sign(
            self.stack.get().unwrap().keystore().private_key(),
            owner,
            peer_id,
            &chunk_id,
            &price,
            cyfs_chunk::ChunkGetReqType::Data,
        )?;

        if let Ok(chunk_resp) = ChunkClient::get_resp_from_source(
            ChunkSourceContext::source_http_local(peer_id),
            &chunk_get_data_req,
        ).await {
            debug!("get chunk {} from local success", &chunk_id);
            async_copy(chunk_resp, writer).await?;
            return Ok(());
        }

        //2. 用HTTP@BDT到Owner取Chunk
        let chunk_get_data_with_meta_req = cyfs_chunk::ChunkGetReq::sign(
            self.stack.get().unwrap().keystore().private_key(),
            owner,
            peer_id,
            &chunk_id,
            &price,
            cyfs_chunk::ChunkGetReqType::Data,
        )?;
        // 由于某些场景中，会发生使用udp的bdt stream在传输一段时间后再也收不到数据的情形，这里加一个超时。如果到了超时时间还没有返回，就再建一条新连接重试一次GetChunk
        // 这里重试3次，3次还得不到chunk就返回错误
        let chunk_content = OnceCell::new();
        let mut get_error = BuckyError::from(BuckyErrorCode::Ok);
        let mut use_tcp = self.config.conn_strategy == ConnStrategy::TcpFirst || self.config.conn_strategy == ConnStrategy::TcpOnly;
        for i in 0..self.config.retry_times {
            info!("try get chunk {}, retry {}/{}", chunk_id, i, self.config.retry_times);
            let mut ctx = None;
            if use_tcp {
                if let StandardObject::Device(device) = self.get_desc(owner.object_id(), None).await? {
                    for endpoint in device.body_expect("").content().endpoints() {
                        if endpoint.is_static_wan() && endpoint.is_tcp() && endpoint.addr().is_ipv4() {
                            info!("named data client use tcp conn to {}", &endpoint.addr().ip().to_string());
                            ctx = Some(ChunkSourceContext::new_http(peer_id, &endpoint.addr().ip().to_string(), self.config.tcp_chunk_manager_port));
                            break;
                        }
                    }
                }
            } else {
                let bdt_stream = self.get_bdt_stream(&owner).await?;
                ctx = Some(ChunkSourceContext::source_http_bdt_remote(peer_id, bdt_stream))
            };

            if ctx.is_none() {
                if self.config.conn_strategy != ConnStrategy::TcpOnly {
                    info!("named data client use tcp, but not found any static tcp ipv4 addr. roolback to bdt");
                    let bdt_stream = self.get_bdt_stream(&owner).await?;
                    ctx = Some(ChunkSourceContext::source_http_bdt_remote(peer_id, bdt_stream))
                } else {
                    info!("named data client use tcp pnly, but not found any static tcp ipv4 addr. return failed");
                    return Err(BuckyError::new(BuckyErrorCode::NotSupport, format!("device {} not has static tcp ipv4 addr", owner)));
                }
            }

            match ChunkClient::get_resp_from_source(ctx.clone().unwrap(), &chunk_get_data_with_meta_req).await {
                Ok(mut resp) => {
                    // 这里超时后返回Timeout错误，再试一次
                    match async_std::future::timeout(self.config.timeout, async {
                        info!("begin read chunk {} data, len {}", chunk_id, chunk_id.len());
                        resp.body_bytes().await
                    }).await {
                        Ok(buf) => {
                            match buf {
                                Ok(buf) => {
                                    info!("end read chunk {} data, len {}", chunk_id, chunk_id.len());
                                    let _ = chunk_content.set(buf);
                                    break;
                                }
                                Err(e) => {
                                    error!("receive bytes error, {}", e);
                                    get_error = BuckyError::from(e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("recv chunk {} timeout after {} secs.", chunk_id, self.config.timeout.as_secs());
                            get_error = BuckyError::from(e);
                        }
                    }
                }
                Err(e) => {
                    error!("get chunk {} response err {}", chunk_id, e);
                }
            }

            // 走到这里说明失败了
            // 这里尝试看能不能让pool放弃这条连接
            if let Some(stream) = ctx.unwrap().get_bdt_stream() {
                if let Err(e) = stream.shutdown(Shutdown::Both) {
                    error!("bdt stream close error! {}", e);
                }
            }

            // 如果是tcp失败，且失败次数超过重试次数的一半，下次用bdt重试
            if self.config.conn_strategy == ConnStrategy::TcpFirst && i+1 > (self.config.retry_times as f32 / 2f32).ceil() as u8 {
                use_tcp = false;
            }
        }

        if chunk_content.get().is_none() {
            // 表示3次都没有拿到chunk数据，这里报超时
            error!("get chunk {} final failed after retry {} times", chunk_id, self.config.retry_times);
            return Err(get_error);
        }

        // 重新计算一次chunkid
        let new_chunk_id = ChunkId::calculate_sync(chunk_content.get().unwrap()).unwrap();
        if chunk_id != &new_chunk_id {
            error!("recalc chunkid failed! except {}, actual {}", &chunk_id, &new_chunk_id);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, "chunkid dismatch"));
        } else {
            debug!("verify chunk {} success", &chunk_id);
        }

        // 存入writer
        debug!("save chunk {} to writer", &chunk_id);
        writer.write(chunk_content.get().unwrap()).await?;

        // 存入本地ChunkManager
        if let Ok((desc, secret)) = cyfs_util::get_default_device_desc() {
            info!("save chunk {} to local", &chunk_id);
            let chunk_set_req = cyfs_chunk::ChunkSetReq::sign(
                &secret,
                &desc.desc().device_id(),
                &chunk_id,
                chunk_content.get().unwrap().to_owned(),
            )?;

            if let Err(e) = ChunkClient::set(
                ChunkSourceContext::source_http_local(peer_id),
                &chunk_set_req,
            ).await {
                warn!("save chunk {} to local fail. err {}", &chunk_id, e)
            }
        }


        return Ok(());
    }

    async fn get_chunks<W: ?Sized>(
        &self,
        chunk_id_list: &Vec<ChunkId>,
        owner: &DeviceId,
        writer: &mut W,
    ) -> BuckyResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        for chunk_id in chunk_id_list {
            self.get_chunk(chunk_id, owner, writer).await?;
        }
        Ok(())
    }

    #[async_recursion::async_recursion]
    async fn get_desc_from_file_manager(&self, id: &ObjectId, owner: &ObjectId) -> BuckyResult<StandardObject> {
        let owner_ood = self.get_device_from_owner_id(owner).await?;
        let use_tcp = self.config.conn_strategy == ConnStrategy::TcpFirst || self.config.conn_strategy == ConnStrategy::TcpOnly;
        if use_tcp {
            if let StandardObject::Device(device) = self.get_desc(owner_ood.object_id(), None).await? {
                for endpoint in device.body_expect("").content().endpoints() {
                    if endpoint.is_static_wan() && endpoint.is_tcp() && endpoint.addr().is_ipv4() {
                        let addr = format!("{}:{}", &endpoint.addr().ip().to_string(), self.config.tcp_file_manager_port);
                        let req = Request::new(Method::Get, format!("http://{}/get_file?fileid={}", &addr, id).as_str());
                        info!("named data client use tcp conn to {}", &addr);
                        let conn = async_std::net::TcpStream::connect(&addr).await?;
                        let mut resp = cyfs_util::async_h1_helper::connect_timeout(conn, req, Duration::from_secs(60 * 5)).await?;
                        let buf = resp.body_bytes().await?;
                        let obj = StandardObject::clone_from_slice(&buf)?;
                        self.object_cache.write().unwrap().insert(id.clone(), obj.clone());
                        return Ok(obj);
                    }
                }
            }
        }

        let req = Request::new(Method::Get, format!("http://www.cyfs.com/file_manager/get_file?fileid={}", id).as_str());
        let conn = self.get_bdt_stream(&owner_ood).await?;
        let mut resp = cyfs_util::async_h1_helper::connect_timeout(conn, req, Duration::from_secs(60 * 5)).await?;
        let buf = resp.body_bytes().await?;
        let obj = StandardObject::clone_from_slice(&buf)?;
        self.object_cache.write().unwrap().insert(id.clone(), obj.clone());
        return Ok(obj);
    }

    #[async_recursion::async_recursion]
    async fn get_desc(
        &self,
        fileid: &ObjectId,
        owner: Option<ObjectId>,
    ) -> BuckyResult<StandardObject> {
        info!("get desc for id {}", fileid);
        //1. 尝试从内存cache里取
        if let Some(obj) = self.object_cache.read().unwrap().get(fileid) {
            return Ok(obj.clone());
        }
        //1. 从FileManager取desc
        /*
        if let Ok(desc) = FileManager::get_desc(fileid).await {
            return Ok(desc);
        }
        */

        //2. 如果找不到，尝试用meta chain查找desc
        info!("try get desc {} from meta", fileid);
        if let Ok(ret) = self.meta_client.get_desc(fileid).await {
            let obj = match ret {
                SavedMetaObject::File(p) => {
                    info!("get file desc {} from meta success", fileid);
                    Ok(StandardObject::File(p))
                },
                SavedMetaObject::People(p) => {
                    info!("get people desc {} from meta success", fileid);
                    Ok(StandardObject::People(p))
                },
                SavedMetaObject::Device(p) => {
                    info!("get device desc {} from meta success", fileid);
                    let device_id = p.desc().device_id();
                    self.device_cache.add(&device_id, p.clone()).await;
                    Ok(StandardObject::Device(p))
                }
                SavedMetaObject::Data(data) => {
                    info!("get desc {} from meta success", &data.id);
                    Ok(StandardObject::clone_from_slice(data.data.as_slice())?)
                }
                _ => {
                    warn!("get desc {} but not file", fileid);
                    Err(BuckyError::from(BuckyErrorCode::NotFound))
                },
            };
            if let Ok(obj) = obj {
                self.object_cache.write().unwrap().insert(fileid.clone(), obj.clone());
                return Ok(obj);
            }
        }

        //3. 如果再找不到，当有owner传入的情况下，用HTTP@BDT到owner去找
        if let Some(owner) = &owner {
            info!("try get desc from owner {}", owner);
            let desc = self.get_desc_from_file_manager(fileid, owner).await?;
            return Ok(desc);
        }

        warn!("cannot get file desc {}", fileid);
        return Err(BuckyError::from(BuckyErrorCode::NotFound));
    }

    pub async fn get_dir(&self, id_str: &str, owner_str: Option<&str>, inner_path: Option<&str>, dest_path: &Path) -> BuckyResult<(Dir, usize)> {
        let id = self.get_id_from_str(id_str).await?;
        let mut owner = None;
        if let Some(str) = owner_str {
            owner = self.get_id_from_str(str).await.map_or(None, |id|Some(id));
        }

        self.get_dir_by_obj(&id, owner, inner_path, dest_path).await
    }

    pub async fn get_dir_by_obj(&self, id: &ObjectId, owner: Option<ObjectId>, inner_path: Option<&str>, dest_path: &Path) -> BuckyResult<(Dir, usize)> {
        info!("get dir by id {}, inner path {}", id, inner_path.unwrap_or("none"));

        let desc = self.get_desc(&id, owner).await?;
        let mut is_file = false;
        if let StandardObject::Dir(dir) = desc {
            match dir.desc().content().obj_list() {
                NDNObjectInfo::ObjList(list) => {
                    let filtred_list;
                    if let Some(inner) = inner_path {
                        let filtered = list.object_map.iter().filter_map(|(path_str, info)| {
                            let path = Path::new(path_str);
                            if path.starts_with(inner) {
                                // 如果能精确匹配上inner_path，就是想下一个单独的文件，这时dest_path就是带文件名的全路径
                                let new_path = if path == Path::new(inner) {
                                    is_file = true;
                                    path
                                } else {
                                    path.strip_prefix(inner).unwrap()
                                };
                                // 这里要改下item的inner_path
                                let new_item = (new_path.to_string_lossy().to_string(), info.clone());
                                Some(new_item)
                            } else {
                                None
                            }
                        }).collect();
                        filtred_list = filtered;
                    } else {
                        filtred_list = list.object_map.clone();
                    }

                    for (path, info) in &filtred_list {
                        match info.node() {
                            InnerNode::ObjId(id) => {
                                let actual_path = if is_file {
                                    dest_path.to_owned()
                                } else {
                                    dest_path.join(path)
                                };

                                if let Some(parent) = actual_path.parent(){
                                    if !parent.exists() {
                                        std::fs::create_dir_all(parent)?;
                                    }
                                }

                                let mut file = async_std::fs::File::create(actual_path).await?;
                                match dir.body_expect("").content() {
                                    DirBodyContent::Chunk(_) => {
                                        error!("dir chunk body not support!");
                                        return Err(BuckyError::from(BuckyErrorCode::NotSupport));
                                    }
                                    DirBodyContent::ObjList(list) => {
                                        if let Some(buf) = list.get(id) {
                                            let file_obj = File::clone_from_slice(buf)?;
                                            self.get_file_by_obj(&file_obj, &mut file).await?;
                                            file.flush().await?;
                                        } else {
                                            error!("cannot find id {} in dir obj!", id);
                                            return Err(BuckyError::from(BuckyErrorCode::NotFound));
                                        }
                                    }
                                }
                                // self.get_file_by_id_obj(id, owner, &mut file).await?;
                                // file.flush().await?;
                            }
                            _ => {
                                warn!("cyfs client not support node type")
                            }
                        }
                    }
                    Ok((dir.clone(), filtred_list.len()))
                },
                NDNObjectInfo::Chunk(_) => {
                    // 先不支持chunk格式
                    error!("Object List in chunk not support");
                    Err(BuckyError::from(BuckyErrorCode::NotSupport))
                }
            }
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotMatch))
        }
    }

    pub async fn get_dir_file<W: ?Sized>(&self, id: &DirId, owner_str: Option<&str>, inner_path: &str, writer: &mut W) -> BuckyResult<File>
        where W: AsyncWrite + Unpin,
    {
        info!("get file by id {}, path {}", id, inner_path);
        let mut owner = None;
        if let Some(str) = owner_str {
            owner = self.get_id_from_str(str).await.map_or(None, |id|Some(id));
        }
        let desc = self.get_desc(id.object_id(), owner).await?;
        if let StandardObject::Dir(dir) = desc {
            match dir.desc().content().obj_list() {
                NDNObjectInfo::ObjList(list) => {
                    if let Some(entry) = list.object_map.get(inner_path) {
                        match entry.node() {
                            InnerNode::ObjId(fileid) => {
                                self.get_file_by_id(&fileid.to_string(), owner_str, writer).await
                            }
                            _ => {
                                Err(BuckyError::from(BuckyErrorCode::NotSupport))
                            }
                        }
                    } else {
                        Err(BuckyError::from(BuckyErrorCode::NotFound))
                    }
                }
                NDNObjectInfo::Chunk(_chunk) => {
                    // 先不支持chunk格式
                    error!("Object List in chunk not support");
                    Err(BuckyError::from(BuckyErrorCode::NotSupport))
                }
            }
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotMatch))
        }
    }

    pub async fn get_file_by_id<W: ?Sized>(
        &self,
        id_str: &str,
        owner_str: Option<&str>,
        writer: &mut W,
    ) -> BuckyResult<File>
        where
            W: AsyncWrite + Unpin,
    {
        info!("get file by id {}", id_str);
        // 先解id，这个必须成功
        let id = self.get_id_from_str(id_str).await?;
        let mut owner = None;
        if let Some(str) = owner_str {
            owner = self.get_id_from_str(str).await.map_or(None, |id|Some(id));
        }

        self.get_file_by_id_obj(&id, owner, writer).await
    }

    // owner可能是peerid或者groupid
    // id和owner也可能是租用的name，这里传str，在内部尝试解析
    pub async fn get_file_by_id_obj<W: ?Sized>(
        &self,
        id: &ObjectId,
        owner: Option<ObjectId>,
        writer: &mut W,
    ) -> BuckyResult<File>
    where
        W: AsyncWrite + Unpin,
    {

        // 取FileDesc
        let desc = self.get_desc(id, owner).await?;
        if let StandardObject::File(desc) = desc {
            info!("get file {} desc success", &id);
            self.get_file_by_obj(&desc, writer).await?;
            Ok(desc.clone())
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotMatch))
        }
    }

    pub async fn get_file_by_obj<W: ?Sized>(&self,
         desc: &File,
         writer: &mut W) -> BuckyResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let owner = desc.desc().owner().unwrap();
        let owner_device = self.get_device_from_owner_id(&owner).await?;
        match desc.body().as_ref().unwrap().content().chunk_list() {
            ChunkList::ChunkInList(list) => {
                info!("now get chunks for file {}:", desc.desc().calculate_id());
                self.get_chunks(&list, &owner_device, writer).await?;
                Ok(())
            }
            ChunkList::ChunkInFile(_fileid) => {
                warn!("chunk in file not supported");
                Err(BuckyError::new(BuckyErrorCode::UnSupport, "ChunkInFile"))
            }
            ChunkList::ChunkInBundle(bundle) => {
                info!("now get chunks for file {}:", desc.desc().calculate_id());
                self.get_chunks(&bundle.chunk_list(), &owner_device, writer).await?;
                Ok(())
            }
        }
    }

    pub async fn put_from_file(
        &mut self,
        source: &Path,
        owner_desc: &StandardObject,
        owner_secret: &PrivateKey,
        chunk_size: u32,
        file_id: Option<PathBuf>,
        save_to_meta: bool
    ) -> BuckyResult<(String, Duration)> {
        if source.is_file() {
            // 如果是单个文件，走文件传输的流程
            let file_desc = generate_file_desc(source, owner_desc, owner_secret, chunk_size, None).await?;
            let fileid = file_desc.desc().calculate_id();

            if let Some(file_id_file) = file_id {
                write_id_to_file(&file_id_file, &fileid);
            }
            let ffs_url = format!("cyfs://{}/{}", file_desc.desc().owner().unwrap(), &fileid);
            let put_dur = self.put(source, &file_desc, owner_desc, owner_secret, save_to_meta, true).await?;
            Ok((ffs_url, put_dur))
        } else if source.is_dir() {
            let (dir_desc, file_descs) = generate_dir_desc_2(source, owner_desc, owner_secret, chunk_size, None).await?;

            let dirid = dir_desc.desc().calculate_id();

            if let Some(file_id_file) = file_id {
                write_id_to_file(&file_id_file, &dirid)
            }
            let mut gen_dur = Duration::new(0, 0);
            let ffs_url = format!("cyfs://{}/{}", dir_desc.desc().owner().unwrap(), &dirid);
            // 把每个文件put到ood上去，这里不需要把每个文件的desc都放到meta链上去
            for (file_desc, abs_path) in file_descs {
                let put_dur = self.put(&abs_path, &file_desc, owner_desc, owner_secret, false, false).await?;
                gen_dur = gen_dur + put_dur;
            }

            // 把dir对象put到ood上去
            let any_dir_obj = AnyNamedObject::Standard(StandardObject::Dir(dir_desc));
            self.put_obj(&any_dir_obj).await?;
            if save_to_meta {
                meta_helper::create_desc(&self.meta_client, &owner_desc, owner_secret
                                         , any_dir_obj).await?;
            }

            Ok((ffs_url, gen_dur))
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotMatch))
        }

    }

    // 如果owner_desc是Device，target_device就是它本身
    // 如果owner_desc是People，target_device就是它的OOD
    // 如果是其他类型，报错退出
    async fn get_device_from_owner_id(&self, owner: &ObjectId) -> BuckyResult<DeviceId> {
        let owner_obj = self.get_desc(owner, None).await?;
        self.get_device_from_owner(&owner_obj).await
    }

    // 如果owner_desc是Device，target_device就是它本身
    // 如果owner_desc是People，target_device就是它的OOD
    // 如果是其他类型，报错退出
    async fn get_device_from_owner(&self, owner: &StandardObject) -> BuckyResult<DeviceId> {
        match owner {
            StandardObject::Device(device) => {
                let device_id = device.desc().device_id();
                self.device_cache.add(&device_id, device.clone()).await;
                Ok(device_id)
            },
            StandardObject::People(people) => {
                let people_id = people.desc().calculate_id();
                let mut device_id = if let StandardObject::People(people) = self.get_desc(&people_id, None).await? {
                    let ood_list = people.body_expect("").content().ood_list();
                    if ood_list.len() > 0 {
                        Ok(ood_list[0].clone())
                    } else {
                        Err(BuckyError::from(BuckyErrorCode::NotFound))
                    }
                } else {
                    Err(BuckyError::from(BuckyErrorCode::NotMatch))
                };
                if device_id.is_err() {
                    let ood_list = people.body_expect("").content().ood_list();
                    if ood_list.len() > 0 {
                        device_id = Ok(ood_list[0].clone())
                    }
                }
                device_id
            },
            _ => {Err(BuckyError::from(BuckyErrorCode::NotSupport))}
        }
    }

    pub async fn put(
        &mut self,
        source: &Path,
        file_desc: &File,
        owner_desc: &StandardObject,
        owner_secret: &PrivateKey,
        save_to_meta: bool,
        put_obj: bool,
    ) -> BuckyResult<Duration> {
        let owner_device = self.get_device_from_owner(owner_desc).await?;
        let start = Instant::now();
        let mut file = std::fs::File::open(source)?;
        let file_ref = file.borrow_mut();
        // 1. 用bdt连接file的owner
        // 当前使用HTTP@BDT，不需要这一步
        // 2. 把chunk存进owner
        match file_desc.body().as_ref().unwrap().content().chunk_list().inner_chunk_list() {
            Some(list) => {
                for chunkid in list {
                    let len = chunkid.len();
                    let mut reader = file_ref.take(len as u64);
                    let mut data = Vec::with_capacity(len as usize);
                    reader.read_to_end(&mut data)?;

                    info!("put chunk {} len {} kB to {}", &chunkid, len / 1024, file_desc.desc().owner().unwrap());

                    let chunk_set_req = cyfs_chunk::ChunkSetReq::sign(
                        owner_secret,
                        &owner_device,
                        &chunkid,
                        data
                    )?;

                    let mut ctx = None;
                    let use_tcp = self.config.conn_strategy == ConnStrategy::TcpFirst || self.config.conn_strategy == ConnStrategy::TcpOnly;
                    if use_tcp {
                        if let StandardObject::Device(device) = self.get_desc(owner_device.object_id(), None).await? {
                            for endpoint in device.body_expect("").content().endpoints() {
                                if endpoint.is_static_wan() && endpoint.is_tcp() && endpoint.addr().is_ipv4() {
                                    info!("named data client use tcp conn to {}", &endpoint.addr().ip().to_string());
                                    ctx = Some(ChunkSourceContext::new_http(&owner_device, &endpoint.addr().ip().to_string(), self.config.tcp_chunk_manager_port));
                                    break;
                                }
                            }
                        }
                    } else {
                        let bdt_stream = self.get_bdt_stream(&owner_device).await?;
                        ctx = Some(ChunkSourceContext::source_http_bdt_remote(&owner_device, bdt_stream))
                    };

                    if ctx.is_none() {
                        if self.config.conn_strategy != ConnStrategy::TcpOnly {
                            info!("named data client use tcp, but not found any static tcp ipv4 addr. roolback to bdt");
                            let bdt_stream = self.get_bdt_stream(&owner_device).await?;
                            ctx = Some(ChunkSourceContext::source_http_bdt_remote(&owner_device, bdt_stream))
                        } else {
                            info!("named data client use tcp pnly, but not found any static tcp ipv4 addr. return failed");
                            return Err(BuckyError::new(BuckyErrorCode::NotSupport, format!("device {} not has static tcp ipv4 addr", &owner_device)));
                        }
                    }

                    let chunk_set_resp = ChunkClient::set(ctx.unwrap(), &chunk_set_req).await?;

                    let public_ley = owner_desc.public_key().unwrap();
                    if let PublicKeyRef::Single(public_key) = public_ley {
                        if !chunk_set_resp.verify(public_key) {
                            return Err(BuckyError::from(BuckyErrorCode::InvalidData));
                        }
                    }


                    info!("put chunk {} to {} success", chunkid, file_desc.desc().owner().unwrap());
                }
            }
            None => {
                return Err(BuckyError::from(BuckyErrorCode::UnSupport));
            }
        }

        if put_obj {
            // 3. 把filedesc存入owner
            self.put_obj(&AnyNamedObject::Standard(StandardObject::File(file_desc.clone()))).await?;
        }


        if save_to_meta {
            // 4. 把filedesc存入meta
            let fileid = file_desc.desc().calculate_id();
            info!("put file {} desc to meta", fileid);
            if let Err(e) = meta_helper::create_file_desc_sync(
                &self.meta_client,
                owner_desc,
                owner_secret,
                &file_desc.clone()
            ).await
            {
                warn!("put file {} desc to meta failed, err {}", fileid, e);
            }
        }

        Ok(start.elapsed())
    }

    // 把对象put到它的owner上去
    pub async fn put_obj(&self, object: &AnyNamedObject) -> BuckyResult<()> {
        let fileid = object.calculate_id();
        let owner_id = object.owner().as_ref().unwrap();
        let owner_device = self.get_device_from_owner_id(owner_id).await?;
        let buf = object.to_vec()?;
        info!("put file desc {} to {}", &fileid, owner_id);

        let use_tcp = self.config.conn_strategy == ConnStrategy::TcpFirst || self.config.conn_strategy == ConnStrategy::TcpOnly;
        if use_tcp {
            if let StandardObject::Device(device) = self.get_desc(owner_device.object_id(), Some(owner_id.clone())).await? {
                for endpoint in device.body_expect("").content().endpoints() {
                    if endpoint.is_static_wan() && endpoint.is_tcp() && endpoint.addr().is_ipv4() {
                        let addr = format!("{}:{}", &endpoint.addr().ip().to_string(), self.config.tcp_file_manager_port);
                        let mut req = Request::new(Method::Post, format!("http://{}/set_file?fileid={}", &addr, &fileid).as_str());
                        req.set_body(buf);
                        info!("named data client use tcp conn to {}", &addr);
                        let conn = async_std::net::TcpStream::connect(&addr).await?;
                        cyfs_util::async_h1_helper::connect_timeout(conn, req, Duration::from_secs(60 * 5)).await?;
                        info!(
                            "put desc {} to {} success",
                            &fileid, &owner_id
                        );
                        return Ok(());
                    }
                }
            }
        }

        let req = Request::new(Method::Post, format!("http://www.cyfs.com/file_manager/set_file?fileid={}", &fileid).as_str());
        let conn = self.get_bdt_stream(&owner_device).await?;
        cyfs_util::async_h1_helper::connect_timeout(conn, req, Duration::from_secs(60 * 5)).await?;
        info!(
            "put desc {} to {} success",
            &fileid, &owner_id
        );

        return Ok(());
    }

    // 从一个cyfs链接解出(owner, file/dirid, inner_path)三个部分
    pub async fn extract_cyfs_url(&self, url: &str) -> BuckyResult<(Option<ObjectId>, ObjectId, String)> {
        let url_str = url.replace("//", "///");
        let url = Url::parse(&url_str)?;

        if url.scheme() != "cyfs" {
            return Err(BuckyError::from(BuckyErrorCode::NotSupport));
        }
        let mut owner = None;
        let mut ndn_id = None;
        let mut inner_path = String::new();
        for path_segment in url.path_segments().ok_or_else(||{BuckyError::from(BuckyErrorCode::NotMatch)})? {
            if owner.is_none() && ndn_id.is_none() {
                // 第一次解析，这个str不是owner就是id
                let id = self.get_id_from_str(path_segment).await?;
                if id.obj_type_code() == ObjectTypeCode::File
                    || id.obj_type_code() == ObjectTypeCode::Dir {
                    ndn_id = Some(id);
                } else {
                    // 先认为非ndn类型的obj id就是owner
                    owner = Some(id);
                }
            } else if ndn_id.is_none() {
                // owner有值，下一个一定是ndn id
                let id = self.get_id_from_str(path_segment).await?;
                if id.obj_type_code() == ObjectTypeCode::File
                    || id.obj_type_code() == ObjectTypeCode::Dir {
                    ndn_id = Some(id);
                } else {
                    // 这个不是ndnid就一定有问题
                    break;
                }
            } else {
                // ndn_id有值，剩下的部分都是inner_path了
                if inner_path.len() > 0 {
                    inner_path.insert_str(inner_path.len(), "/");
                }

                inner_path.insert_str(inner_path.len(), path_segment);
            }

        }

        Ok((owner, ndn_id.ok_or_else(||{BuckyError::from(BuckyErrorCode::InvalidFormat)})?, inner_path))
    }
}


