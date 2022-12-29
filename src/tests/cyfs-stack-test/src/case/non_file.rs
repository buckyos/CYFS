use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

use std::borrow::Cow;
use std::convert::TryFrom;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage non files dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

pub async fn test() {
    let dec_id = new_dec("test-non-file");

    let (dir_id, _file_id) = add_dir(&dec_id).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    list_dir(&stack, dir_id.clone()).await;

    info!("test all non file case success!");
}

async fn add_dir(_dec_id: &ObjectId) -> (DirId, FileId) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("root");

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let req = TransPublishFileOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: None,
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: data_dir.clone(),

        // chunk大小
        chunk_size: 1024 * 1024,

        chunk_method: TransPublishChunkMethod::Track,

        access: None,

        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(req).await;
    if ret.is_err() {
        error!("trans add_dir error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("trans add dir success! id={}", resp.file_id);

    let dir_id = DirId::try_from(&resp.file_id).unwrap();

    let object = {
        let mut req = NONGetObjectRequest::new_noc(
            dir_id.object_id().to_owned(),
            Some("/test1/cyfs-stack-test_53808_rCURRENT.log".to_owned()),
        );
        req.common.req_path = Some("/tests/non_file".to_owned());

        let resp = stack.non_service().get_object(req).await.unwrap();
        resp.object
    };

    let file_id = FileId::try_from(&object.object_id).unwrap();
    (dir_id, file_id)
}

async fn list_dir(stack: &SharedCyfsStack, root: DirId) {
    use std::collections::VecDeque;
    let mut pending_inner_path = VecDeque::new();
    pending_inner_path.push_back("/".to_owned());

    // 从root dir出发
    loop {
        if pending_inner_path.is_empty() {
            break;
        }

        // 列举目录需要以/*结尾，比如/*, test1/*等
        let inner_path = pending_inner_path.pop_front().unwrap();
        let query_inner_path = inner_path.trim_end_matches('/').to_owned() + "/*";
        info!("will list dir: {}", query_inner_path);
        let mut req = NONGetObjectRequest::new_noc(
            root.object_id().to_owned(),
            Some(query_inner_path.clone()),
        );

        req.common.req_path = Some("/tests/list_dir".to_owned());
        req.common.flags |= cyfs_lib::CYFS_REQUEST_FLAG_LIST_DIR;

        let resp = stack.non_service().get_object(req).await.unwrap();
        match resp.object.object_id.obj_type_code() {
            ObjectTypeCode::File => {
                // 获取到一个叶子节点，打印文件信息
                info!(
                    "list dir got file: inner_path={}, id={}",
                    inner_path, resp.object.object_id
                );
            }
            ObjectTypeCode::Dir => {
                info!(
                    "list dir got dir: inner_path={}, id={}",
                    inner_path, resp.object.object_id
                );
                let dir = Dir::clone_from_slice(&resp.object.object_raw).unwrap();
                let entries: Cow<NDNObjectList> = match dir.desc().content().obj_list() {
                    NDNObjectInfo::ObjList(entries) => Cow::Borrowed(entries),
                    NDNObjectInfo::Chunk(chunk_id) => {
                        let list = dir.get_data_from_body(&chunk_id.object_id());
                        match list {
                            Some(data) => {
                                match NDNObjectList::clone_from_slice(&data) {
                                    Ok(entries) => Cow::Owned(entries),
                                    Err(e) => {
                                        error!(
                                            "invalid dir entries: inner_path={}, {}",
                                            inner_path, e
                                        );
                                        // 解码失败，认为是一个错误的dir，不再处理此dir
                                        continue;
                                    }
                                }
                            }
                            None => {
                                // body里面不存在这种dir，暂时不处理这种情况
                                continue;
                            }
                        }
                    }
                };

                let mut sub_dirs = std::collections::HashSet::new();
                for (k, v) in &entries.object_map {
                    // 路径存在压缩模式，所以这里判断一次，对于压缩模式，只取第一段，剩余部分在展开这级目录时候再向non获取吧。。
                    let mut segs: VecDeque<&str> = k.split("/").collect();
                    if segs.len() > 1 {
                        // 获取到一个子dir
                        let sub_dir_path = segs.pop_front().unwrap();

                        // 压缩模式下需要去重
                        // 比如存在/a/b/c, /a/b/d情况，
                        if sub_dirs.insert(sub_dir_path.to_owned()) {
                            info!(
                                "list dir got sub dir: inner_path={}, name={}",
                                inner_path, sub_dir_path
                            );
                            let full_inner_path =
                                inner_path.trim_end_matches('/').to_owned() + "/" + sub_dir_path;
                            pending_inner_path.push_back(full_inner_path);
                        }
                    } else {
                        // 终于到了叶子节点。。记录叶子节点
                        // 判断节点类型，分为file和dir
                        match v.node() {
                            InnerNode::ObjId(id) => {
                                match id.obj_type_code() {
                                    ObjectTypeCode::File => {
                                        // 遍历到一个file
                                        info!(
                                            "list dir got file: inner_path={}, name={}",
                                            inner_path, k
                                        );
                                    }
                                    ObjectTypeCode::Dir => {
                                        // 又获取到一个内嵌的dir。。。，只能再使用/*命令向协议栈发起list请求了
                                        info!(
                                            "list dir got sub dir: inner_path={}, name={}",
                                            inner_path, k
                                        );

                                        let full_inner_path =
                                            inner_path.trim_end_matches('/').to_owned() + k;
                                        pending_inner_path.push_back(full_inner_path);
                                    }
                                    _ => {
                                        // 其余类型先不支持
                                    }
                                }
                            }
                            InnerNode::Chunk(chunk_id) => {
                                // 单文件file
                                info!(
                                    "list dir got file: inner_path={}, chunk={}, name={}",
                                    inner_path, chunk_id, k
                                );
                            }
                            _ => {
                                unreachable!();
                            }
                        }
                    }
                }
            }
            _ => {
                unreachable!();
            }
        }
    }
}
async fn get_by_zone_device() {}
