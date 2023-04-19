use crate::ffs_client_util::{generate_file_desc, write_id_to_file, generate_dir_desc_2};
use crate::meta_helper;
use async_std::io::{copy as async_copy};
use async_std::io::Write as AsyncWrite;
use async_std::prelude::*;
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
use std::sync::{Arc, RwLock};
use cyfs_bdt::stream_pool::{PooledStream, StreamPool};
use once_cell::sync::OnceCell;
use cyfs_bdt::{StackGuard, DeviceCache};
use cyfs_lib::{NDNGetDataRequest, NDNPutDataRequest, NONGetObjectRequest, NONPutObjectRequest, UniCyfsStackRef};

pub struct NamedCacheClient {
    meta_client: Arc<MetaClient>,
    config: NamedCacheClientConfig,
    cyfs_stack: UniCyfsStackRef
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
    pub fn new(config: NamedCacheClientConfig, uni_stack: UniCyfsStackRef)  -> NamedCacheClient {
        let client = Arc::new(MetaClient::new_target(config.meta_target.clone()).with_timeout(Duration::from_secs(60 * 2)));
        NamedCacheClient {
            meta_client: client,
            config,
            cyfs_stack: uni_stack
        }
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
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

    pub fn reset_known_sn_list(&self, sn_list: Vec<Device>) -> BuckyResult<()> {
        if let Some(stack) = self.stack.get() {
            info!("named data client reset sn list {:?}", sn_list.iter().map(|device|{
                device.desc().calculate_id()
            }).collect::<Vec<ObjectId>>());
            stack.reset_known_sn(sn_list);
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

    async fn get_chunk<W: ?Sized>(
        &self,
        chunk_id: &ChunkId,
        owner: &ObjectId,
        writer: &mut W,
    ) -> BuckyResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        debug!("will get chunk by id {}", &chunk_id);

        let request = NDNGetDataRequest::new_router(Some(owner.clone()), chunk_id.object_id(), None);
        let data_resp = self.cyfs_stack.ndn_service().get_data(request).await?;

        async_copy(data_resp.data, writer).await?;
        Ok(())
    }

    async fn get_chunks<W: ?Sized>(
        &self,
        chunk_id_list: &Vec<ChunkId>,
        owner: &ObjectId,
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

    async fn get_desc(
        &self,
        fileid: &ObjectId,
        owner: Option<ObjectId>,
    ) -> BuckyResult<StandardObject> {
        info!("get desc for id {}", fileid);
        let resp = self.cyfs_stack.non_service().get_object(NONGetObjectRequest::new_router(owner, fileid.clone(), None)).await?;
        let obj = StandardObject::clone_from_slice(&resp.object.object_raw)?;
        Ok(obj)
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
        match desc.body().as_ref().unwrap().content().chunk_list() {
            ChunkList::ChunkInList(list) => {
                info!("now get chunks for file {}:", desc.desc().calculate_id());
                self.get_chunks(&list, &owner, writer).await?;
                Ok(())
            }
            ChunkList::ChunkInFile(_fileid) => {
                warn!("chunk in file not supported");
                Err(BuckyError::new(BuckyErrorCode::UnSupport, "ChunkInFile"))
            }
            ChunkList::ChunkInBundle(bundle) => {
                info!("now get chunks for file {}:", desc.desc().calculate_id());
                self.get_chunks(&bundle.chunk_list(), &owner, writer).await?;
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
        let owner = file_desc.desc().owner().as_ref().unwrap();
        let start = Instant::now();
        let mut file = async_std::fs::File::open(source).await?;
        let file_ref = file.borrow_mut();
        // 2. 把chunk存进owner
        match file_desc.body().as_ref().unwrap().content().chunk_list().inner_chunk_list() {
            Some(list) => {
                for chunkid in list {
                    let len = chunkid.len();
                    let mut reader = file_ref.take(len as u64);
                    let mut data = Vec::with_capacity(len as usize);
                    reader.read_to_end(&mut data)?;

                    info!("put chunk {} len {} kB to {}", &chunkid, len / 1024, owner);

                    let resp = self.cyfs_stack.ndn_service().put_data(
                        NDNPutDataRequest::new_router(Some(owner_desc.calculate_id()),
                                                      chunkid.object_id(),
                                                      len as u64,
                                                      Box::new(reader))).await?;


                    info!("put chunk {} to {} success, result {}", chunkid, owner, resp.result.to_string());
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
        let mut req = NONPutObjectRequest::new_router(Some(owner_id.clone()), fileid, object.to_vec()?);
        req.access = Some(AccessString::full());
        let resp = self.cyfs_stack.non_service().put_object(req).await?;
        info!(
            "put desc {} to {} success, result {}",
            &fileid, owner_id, resp.result.to_string()
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


