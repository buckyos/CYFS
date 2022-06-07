use crate::acl::*;
use crate::ndn::*;
use crate::non::*;
use crate::resolver::OodResolver;
use cyfs_base::*;
use cyfs_lib::*;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

enum DirSearchResult {
    Dir((Dir, Attributes)),
    File((File, Attributes)),
    FileId((FileId, Attributes)),
}

// dir+inner_path，对应一个sub_dir/file/单chunk文件
pub enum DirResult {
    Dir((Dir, Attributes)),
    File((File, Attributes)),
}

pub(crate) struct NONDirLoader {
    non_api_level: NONAPILevel,
    non_processor: NONInputProcessorRef,
    ndn_processor: NDNInputProcessorRef,
    ood_resolver: OodResolver,
}

impl NONDirLoader {
    pub fn new(
        non_api_level: NONAPILevel,
        non_processor: NONInputProcessorRef,
        ndn_processor: NDNInputProcessorRef,
        ood_resolver: OodResolver,
    ) -> Self {
        Self {
            non_api_level,
            non_processor,
            ndn_processor,
            ood_resolver,
        }
    }

    pub async fn get_dir(&self, req: NONGetObjectInputRequest) -> BuckyResult<DirResult> {
        use std::collections::VecDeque;

        // 转换到dir_id
        let mut dir_id: DirId = DirId::try_from(&req.object_id).unwrap();
        let mut dir_attr: Attributes = Attributes::default();

        // 解析inner_path
        let mut segs: VecDeque<String> = if let Some(inner_path) = &req.inner_path {
            inner_path
                .split("/")
                .filter_map(|v| {
                    if v.is_empty() {
                        /*
                        trace!(
                            "dir inner_path got empty seg path! dir={}, inner_path={}",
                            dir_id, inner_path
                        );
                        */
                        None
                    } else {
                        Some(v.to_owned())
                    }
                })
                .collect()
        } else {
            VecDeque::new()
        };

        info!("will get dir: {}", req);

        // dir对象尝试使用多target
        let mut targets = vec![];
        if let Some(target) = &req.common.target {
            targets.push(target.to_owned());
        }

        // 从请求的dir开始的根引用
        let mut root_referer_object = NDNDataRefererObject {
            object_id: req.object_id.clone(),
            inner_path: None,
        };
        let mut inner_path_parts = vec![];

        // 从root开始，递归查找
        let mut level: u32 = 0;
        let result = loop {
            level = level + 1;
            info!(
                "will get dir: id={}, source={}, target={:?}, level={}",
                dir_id, req.common.source, req.common.target, level
            );

            // 首先从目标target查找该层级的dir对象
            let resp = self
                .get_object_from_targets(
                    &req.common,
                    dir_id.object_id(),
                    &targets,
                    &root_referer_object,
                )
                .await?;

            if resp.object.object_id.obj_type_code() != ObjectTypeCode::Dir {
                let msg = format!(
                    "invalid dir object type: id={}, {:?}",
                    dir_id,
                    resp.object.object_id.obj_type_code()
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            // 如果是dir，并且没有指定target，那么从root dir的owner尝试发现可能的目标对象
            // TODO 如果指定了target，那么是不是也要尝试从dir_owner去加载内部的子对象和chunk?
            // 如果是noc层，那么不要去查找target了，只能从本地noc查找
            if level == 1 && self.non_api_level != NONAPILevel::NOC && targets.is_empty() {
                self.search_target(
                    dir_id.object_id(),
                    resp.object.object.as_ref().unwrap(),
                    &mut targets,
                )
                .await;
            }

            //let obj: AnyNamedObject = resp.object.into();
            let dir_obj = if let AnyNamedObject::Standard(StandardObject::Dir(dir_obj)) =
                resp.object.object.unwrap().into()
            {
                dir_obj
            } else {
                unreachable!();
            };

            // 没有内部路径了，终止查找
            if segs.is_empty() {
                break DirSearchResult::Dir((dir_obj, dir_attr));
            }

            // 存在内部路径，继续查找
            // 首先获取dir的头部的列表信息，需要处理两种格式
            #[allow(unused_assignments)]
            let mut origin_entries = None;
            let entries = match dir_obj.desc().content().obj_list() {
                NDNObjectInfo::ObjList(entries) => entries,
                NDNObjectInfo::Chunk(chunk_id) => {
                    // desc里面依赖的chunk需要从body或者targets里面查找
                    let buf = self
                        .get_desc_chunk_from_body_and_targets(
                            &req.common,
                            &dir_id,
                            &dir_obj,
                            chunk_id.clone(),
                            &targets,
                            root_referer_object.clone(),
                        )
                        .await?;
                    let (entries, _) = NDNObjectList::raw_decode(&buf).map_err(|e| {
                        let msg = format!(
                            "decode NDNObjectList from chunk error! dir={}, chunk={}, {}",
                            dir_id, chunk_id, e
                        );
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                    })?;
                    origin_entries = Some(entries);
                    origin_entries.as_ref().unwrap()
                }
            };

            // 首先查找剩余的完成路径，按照最长查找原则
            // 可能存在一段或者完整的子路径
            let left_path = segs.iter().fold(String::new(), |a, b| {
                if a.len() == 0 {
                    a + b
                } else {
                    a + "/" + b
                }
            });

            // 看是不是存在压缩模式的子路径，比如{dir_id}/a/b
            let entry = match entries.object_map().get(&left_path) {
                Some(item) => {
                    debug!(
                        "inner path found in dir desc: dir={}, inner_path={}",
                        dir_id, left_path
                    );

                    // 压缩模式子路径
                    inner_path_parts.push(left_path);
                    root_referer_object.inner_path = Some(inner_path_parts.join("/"));

                    segs.clear();
                    item
                }
                None => {
                    // 递归查找第一级子路径
                    let seg = segs.pop_front().unwrap();

                    match entries.object_map().get(&seg) {
                        Some(item) => {
                            // 单段子路径模式
                            inner_path_parts.push(seg);
                            root_referer_object.inner_path = Some(inner_path_parts.join("/"));

                            item
                        }
                        None => {
                            // 判断是否要尝试收集此级目录的子列表
                            if req.common.flags & CYFS_REQUEST_FLAG_LIST_DIR == 0 {
                                let msg = format!(
                                    "dir inner path not found! dir={}, inner path={} or {}",
                                    dir_id, seg, left_path
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InnerPathNotFound, msg));
                            }

                            // 处理以/*结尾的情况
                            let left_path = left_path.trim_end_matches('*').to_owned();
                            let sub_path = if !left_path.ends_with('/') {
                                left_path + "/"
                            } else {
                                left_path
                            };

                            let sub_path = if !sub_path.starts_with('/') {
                                format!("/{}", sub_path)
                            } else {
                                sub_path
                            };

                            let mut object_list = NDNObjectList::new(None);
                            for (name, node) in entries.object_map() {
                                // 确保子路径以/开始，方便统一的前缀匹配
                                let name = if name.starts_with('/') {
                                    std::borrow::Cow::Borrowed(name)
                                } else {
                                    std::borrow::Cow::Owned(format!("/{}", name))
                                };

                                if name.starts_with(&sub_path) {
                                    let left = name.trim_start_matches(&sub_path);
                                    debug!("got sub dir/file path! name={}, sub_path={}, left={}", name, sub_path, left);
                                    object_list
                                        .object_map
                                        .insert(left.to_owned(), node.to_owned());
                                }
                            }

                            // 如果一个条目也没有，说明不存在对应的子目录
                            if object_list.object_map.is_empty() {
                                let msg = format!(
                                    "dir inner dir not exists! dir={}, inner path={}",
                                    dir_id, sub_path
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InnerPathNotFound, msg));
                            }

                            // 创建一个降级的dir对象
                            let builder = Dir::new(
                                Attributes::default(),
                                NDNObjectInfo::ObjList(object_list),
                                HashMap::new(),
                            );
                            let builder = builder.no_create_time();
                            let builder = if let Some(owner) = dir_obj.desc().owner() {
                                builder.owner(owner.to_owned())
                            } else {
                                builder
                            };

                            let mut dir = builder.build();
                            dir.check_and_fix_desc_limit()?;

                            break DirSearchResult::Dir((dir, Attributes::default()));
                        }
                    }
                }
            };

            match entry.node() {
                InnerNode::ObjId(id) => {
                    match id.obj_type_code() {
                        ObjectTypeCode::File => {
                            // 如果当前节点是文件，但还需要继续查找下一级路径，那么返回失败
                            if !segs.is_empty() {
                                let msg = format!(
                                    "file inner path not support! root dir={}, inner_path={:?}, file={}, left_inner_path={:?}",
                                    req.object_id, root_referer_object.inner_path, id, segs
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InnerPathNotFound, msg));
                            }

                            let file_id = FileId::try_from(id).unwrap();

                            let ret = self
                                .get_file_from_dir(
                                    &req.common,
                                    &dir_id,
                                    &dir_obj,
                                    file_id,
                                    entry.attributes().to_owned(),
                                    &targets,
                                    &root_referer_object,
                                )
                                .await?;

                            break ret;
                        }
                        ObjectTypeCode::Dir => {
                            // 子路径，继续查找
                            dir_id = DirId::try_from(id).unwrap();
                            dir_attr = entry.attributes().to_owned();

                            continue;
                        }
                        ObjectTypeCode::Diff => {
                            unreachable!("diff object not support!");
                        }
                        _ => {
                            let msg = format!(
                                "dir inner node object type not support! root_dir={}, inner_path={:?}, cur_dir={}, id={}, left_inner_path={:?}",
                                req.object_id, root_referer_object.inner_path, dir_id, id, segs
                            );
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                        }
                    }
                }
                InnerNode::Chunk(chunk_id) => {
                    // 单文件chunk
                    info!(
                        "got single chunk file! root_dir={}, inner_path={:?}, cur_dir={}, chunk={}",
                        req.object_id, root_referer_object.inner_path, dir_id, chunk_id,
                    );

                    // 不能再有下一级子路径了
                    if !segs.is_empty() {
                        let msg = format!(
                            "single chunk file inner path not support! root_dir={}, inner_path={:?}, cur_dir={}, chunk={}, left_inner_path={:?}",
                            req.object_id, root_referer_object.inner_path, dir_id, chunk_id, segs
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InnerPathNotFound, msg));
                    }

                    // 创建一个单chunk的file对象
                    let builder = File::new_no_owner(
                        chunk_id.len() as u64,
                        HashValue::from(chunk_id.hash()),
                        ChunkList::ChunkInList(vec![chunk_id.to_owned()]),
                    );
                    let builder = builder.no_create_time();
                    let builder = if let Some(owner) = dir_obj.desc().owner() {
                        builder.owner(owner.to_owned())
                    } else {
                        builder
                    };
                    let file = builder.build();

                    break DirSearchResult::File((file, entry.attributes().to_owned()));
                }
                InnerNode::IndexInParentChunk(..) => {
                    unimplemented!();
                }
            }
        };

        let ret = match result {
            DirSearchResult::Dir(ret) => DirResult::Dir(ret),
            DirSearchResult::File(ret) => DirResult::File(ret),
            DirSearchResult::FileId((file_id, attr)) => {
                let file = self
                    .get_file_from_targets(&req.common, &file_id, &targets, &root_referer_object)
                    .await?;
                DirResult::File((file, attr))
            }
        };

        Ok(ret)
    }

    // 从targets尝试加载file对象
    pub async fn get_file_from_targets(
        &self,
        common: &NONInputRequestCommon,
        file_id: &FileId,
        targets: &Vec<ObjectId>,
        referer_object: &NDNDataRefererObject,
    ) -> BuckyResult<File> {
        info!(
            "will get file: id={}, targets={:?}, rerfer={:?}",
            file_id, targets, referer_object
        );

        // 尝试从所有源查询file对象
        let file = self
            .get_object_from_targets(
                &common,
                file_id.object_id(),
                targets.as_ref(),
                referer_object,
            )
            .await?;

        if file.object.object_id.obj_type_code() != ObjectTypeCode::File {
            let msg = format!(
                "invalid object id type, file excepted: id={}, {:?}",
                file_id,
                file.object.object_id.obj_type_code()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        if let AnyNamedObject::Standard(StandardObject::File(file_obj)) =
            file.object.object.unwrap().into()
        {
            Ok(file_obj)
        } else {
            unreachable!();
        }
    }

    async fn get_object_from_targets(
        &self,
        common: &NONInputRequestCommon,
        object_id: &ObjectId,
        targets: &Vec<ObjectId>,
        referer_object: &NDNDataRefererObject,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if targets.is_empty() {
            self.get_object_from_target(common, object_id, None, referer_object)
                .await
        } else {
            for target in targets {
                if let Ok(data) = self
                    .get_object_from_target(common, object_id, Some(target), referer_object)
                    .await
                {
                    return Ok(data);
                }
            }

            let msg = format!(
                "get object from targets failed! object={}, targets={:?}",
                object_id, targets
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }

    async fn get_object_from_target(
        &self,
        common: &NONInputRequestCommon,
        object_id: &ObjectId,
        target: Option<&ObjectId>,
        referer_object: &NDNDataRefererObject,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let mut common = common.to_owned();
        common.target = target.cloned();

        // 如果存在内部路径，需要附加到req_path里面
        if referer_object.object_id != *object_id {
            let left_path = AclResource::join(
                &common.req_path,
                &Some(referer_object.object_id),
                &referer_object.inner_path,
            );
            common.req_path = Some(left_path);
        }

        let get_req = NONGetObjectInputRequest {
            common,
            object_id: object_id.to_owned(),
            inner_path: None,
        };

        debug!("will non get object: {}", get_req);
        self.non_processor.get_object(get_req).await
    }

    // 从对象本身(owner)，尝试推导target
    async fn search_target(
        &self,
        object_id: &ObjectId,
        object: &Arc<AnyNamedObject>,
        targets: &mut Vec<ObjectId>,
    ) {
        match self
            .ood_resolver
            .get_ood_by_object(object_id.to_owned(), None, object.clone())
            .await
        {
            Ok(list) => {
                if list.is_empty() {
                    info!(
                        "get targets from file|dir owner but not found! file={}, owner={:?}",
                        object_id,
                        object.owner()
                    );
                } else {
                    info!(
                        "get targets from file|dir owner! file={}, owner={:?}, sources={:?}",
                        object_id,
                        object.owner(),
                        list
                    );

                    list.into_iter().for_each(|device_id| {
                        // 这里需要列表去重
                        let object_id = device_id.into();
                        if !targets.iter().any(|v| *v == object_id) {
                            targets.push(object_id);
                        }
                    });
                }
            }
            Err(_) => {
                error!(
                    "get targets from file|dir owner failed! file={}, owner={:?}",
                    object_id,
                    object.owner()
                );
            }
        }
    }

    // dir desc里面的chunk，存在两种情况:
    // 1. 在body的缓存里面
    // 2. 从源发起NDN查找
    async fn get_desc_chunk_from_body_and_targets(
        &self,
        common: &NONInputRequestCommon,
        dir_id: &DirId,
        dir_obj: &Dir,
        chunk_id: ChunkId,
        targets: &Vec<ObjectId>,
        root_referer_object: NDNDataRefererObject,
    ) -> BuckyResult<Vec<u8>> {
        // 首先从body里面查找
        if let Some(buf) = self
            .get_object_from_dir(
                common,
                dir_id,
                dir_obj,
                &chunk_id.object_id(),
                targets,
                root_referer_object.clone(),
            )
            .await?
        {
            info!(
                "get chunk from dir body: dir={}, chunk={}",
                dir_id, chunk_id
            );
            return Ok(buf);
        }

        // 直接从源发起查找
        // 添加最近的refer
        let latest_referer_object = NDNDataRefererObject {
            object_id: dir_id.object_id().to_owned(),
            inner_path: None,
        };
        let mut referer_object = vec![root_referer_object, latest_referer_object];
        referer_object.dedup();

        self.get_chunk_from_targets(common, &chunk_id, targets, referer_object)
            .await
    }

    // 尝试通过NDN从targets加载一个chunk
    async fn get_chunk(
        &self,
        common: &NONInputRequestCommon,
        chunk_id: &ChunkId,
        target: Option<&ObjectId>,
        referer_object: Vec<NDNDataRefererObject>,
    ) -> BuckyResult<Vec<u8>> {
        let ndn_common = NDNInputRequestCommon {
            req_path: common.req_path.clone(),
            dec_id: common.dec_id.clone(),
            source: common.source.clone(),
            protocol: common.protocol.clone(),
            target: target.cloned(),
            level: self.non_api_level.clone().into(),
            referer_object,
            flags: common.flags,
            user_data: None,
        };

        let get_req = NDNGetDataInputRequest {
            common: ndn_common,
            object_id: chunk_id.object_id().to_owned(),
            data_type: NDNDataType::Mem,
            range: None,
            inner_path: None,
        };

        let mut resp = self.ndn_processor.get_data(get_req).await?;

        use async_std::io::ReadExt;

        let mut buf = vec![];
        resp.data.read_to_end(&mut buf).await.map_err(|e| {
            let msg = format!("read chunk data failed! chunk={}, {}", chunk_id, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        Ok(buf)
    }

    // 尝试从一组targets加载chunk
    async fn get_chunk_from_targets(
        &self,
        common: &NONInputRequestCommon,
        chunk_id: &ChunkId,
        targets: &Vec<ObjectId>,
        referer_object: Vec<NDNDataRefererObject>,
    ) -> BuckyResult<Vec<u8>> {
        if targets.is_empty() {
            self.get_chunk(common, chunk_id, None, referer_object).await
        } else {
            for target in targets {
                if let Ok(data) = self
                    .get_chunk(common, chunk_id, Some(target), referer_object.clone())
                    .await
                {
                    return Ok(data);
                }
            }

            let msg = format!(
                "get chunk from targets failed! chunk={}, targets={:?}",
                chunk_id, targets
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }

    // 从body的对象缓存里面查询指定的对象
    // 对于{dir_id}/a/b/c，查找c里面的object，那么root_referer就是{dir_id}/a/b，latest_referer就是c
    async fn get_object_from_dir(
        &self,
        common: &NONInputRequestCommon,
        dir_id: &DirId,
        dir_obj: &Dir,
        object_id: &ObjectId,
        targets: &Vec<ObjectId>,
        root_referer_object: NDNDataRefererObject,
    ) -> BuckyResult<Option<Vec<u8>>> {
        match dir_obj.body() {
            Some(body) => {
                match body.content() {
                    DirBodyContent::Chunk(chunk_id) => {
                        // dirbody又打包到了一个chunk，所以这里要再次使用NDN发起一次chunk查询
                        // 需要从chunk里面decode出来obj_list

                        // 加入最近一级引用
                        let latest_referer_object = NDNDataRefererObject {
                            object_id: dir_id.object_id().to_owned(),
                            inner_path: None,
                        };
                        let mut referer_object = vec![root_referer_object, latest_referer_object];
                        referer_object.dedup();

                        let buf = self
                            .get_chunk_from_targets(&common, chunk_id, targets, referer_object)
                            .await
                            .map_err(|e| {
                                let msg = format!(
                                    "load dir body chunk failed! dir={}, chunk={} {}",
                                    dir_id, chunk_id, e
                                );
                                error!("{}", msg);
                                BuckyError::new(e.code(), msg)
                            })?;

                        let obj_list =
                            DirBodyContentObjectList::clone_from_slice(&buf).map_err(|e| {
                                let msg = format!(
                                "decode obj list from buf in dir body failed! dir={}, file={}, {}",
                                dir_id, object_id, e
                            );
                                error!("{}", msg);
                                BuckyError::new(BuckyErrorCode::InvalidData, msg)
                            })?;

                        match obj_list.get(object_id) {
                            Some(buf) => Ok(Some(buf.to_owned())),
                            None => Ok(None),
                        }
                    }
                    DirBodyContent::ObjList(obj_list) => match obj_list.get(object_id) {
                        Some(buf) => Ok(Some(buf.to_owned())),
                        None => Ok(None),
                    },
                }
            }
            None => Ok(None),
        }
    }

    async fn get_file_from_dir(
        &self,
        common: &NONInputRequestCommon,
        dir_id: &DirId,
        dir_obj: &Dir,
        file_id: FileId,
        attr: Attributes,
        targets: &Vec<ObjectId>,
        referer_object: &NDNDataRefererObject,
    ) -> BuckyResult<DirSearchResult> {
        let ret = match self
            .get_object_from_dir(
                common,
                dir_id,
                dir_obj,
                file_id.object_id(),
                targets,
                referer_object.to_owned(),
            )
            .await?
        {
            Some(buf) => {
                let file = File::clone_from_slice(&buf).map_err(|e| {
                    let msg = format!(
                        "decode file from buf in dir body failed! dir={}, file={}, {}",
                        dir_id, file_id, e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

                DirSearchResult::File((file, attr))
            }
            None => DirSearchResult::FileId((file_id, attr)),
        };

        Ok(ret)
    }
}
