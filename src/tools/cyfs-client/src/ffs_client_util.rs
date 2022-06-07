use std::path::{Path, PathBuf};
use async_std::prelude::*;

use log::*;
use cyfs_base::{BuckyResult, PrivateKey, hash_file, ChunkList, ChunkId, ObjectId, File, RsaCPUObjectSigner, sign_and_set_named_object, SignatureSource, hash_data, Dir, BuckyError, BuckyErrorCode, Attributes, NDNObjectList, InnerNode, RawConvertTo, SIGNATURE_SOURCE_REFINDEX_OWNER, InnerNodeInfo, NDNObjectInfo, StandardObject, NamedObject, ObjectDesc};
use std::io::Write;
use std::collections::{HashMap, VecDeque};

async fn create_chunk_list(file: &Path, chunk_size: u32, save_path: Option<PathBuf>) -> BuckyResult<ChunkList> {
    let mut list = Vec::<ChunkId>::new();
    let mut file = async_std::fs::File::open(file).await?;
    let mut buf = vec![];
    buf.resize(chunk_size as usize, 0);
    loop {
        let len = file.read(&mut buf).await?;
        if len == 0 {
            break
        }
        let hash = hash_data(&buf[0..len]);
        let chunkid = ChunkId::new(&hash, len as u32);
        debug!("create chunkid: {}", &chunkid);

        if let Some(path) = &save_path {
            let path = path.join(chunkid.to_string());
            let mut file = std::fs::File::create(&path).unwrap();
            if let Err(e) = file.write(&buf[0..len]) {
                let msg = format!("write file error! file={}, bytes={}, {}", path.display(), len, e);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        list.push(chunkid);
    }

    debug!("create {} chunks", list.len());
    Ok(ChunkList::ChunkInList(list))
}

pub async fn generate_dir_desc_2(source: &Path, owner_desc: &StandardObject, owner_secret: &PrivateKey, chunk_size: u32, save_path: Option<PathBuf>) -> BuckyResult<(Dir, Vec<(File, PathBuf)>)> {
    if !source.is_dir() {
        return Err(BuckyError::from(BuckyErrorCode::NotMatch));
    }

    let mut entrys = HashMap::new();
    let mut bodys = HashMap::new();
    let mut files = vec![];
    for entry_ret in walkdir::WalkDir::new(source) {
        match entry_ret {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    let rel_path = entry.path().strip_prefix(source)?.to_string_lossy().to_string().replace("\\", "/");
                    info!("walk file {}, inner {}", entry.path().display(), &rel_path);
                    // 添加文件到Dir
                    let file = generate_file_desc(entry.path(), owner_desc, owner_secret, chunk_size, save_path.clone()).await?;
                    let file_id = file.desc().calculate_id();
                    entrys.insert(rel_path.clone(), InnerNodeInfo::new(Attributes::new(0), InnerNode::ObjId(file_id.clone())));
                    bodys.insert(file_id.clone(), file.to_vec()?);
                    files.push((file, entry.path().to_owned()));
                }
            }
            Err(e) => {
                error!("walk dir {} err {}", source.display(), e);
            }
        }
    }

    let mut dir = Dir::new(Attributes::new(0), NDNObjectInfo::ObjList(NDNObjectList {
        parent_chunk: None,
        object_map: entrys
    }), bodys).create_time(0).owner(owner_desc.calculate_id()).build();

    // 给dir和files签名
    let signer = RsaCPUObjectSigner::new(owner_secret.public(), owner_secret.clone());
    sign_and_set_named_object(&signer, &mut dir, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER)).await?;

    Ok((dir, files))
}

pub async fn generate_dir_desc(source: &Path, owner_desc: &StandardObject, owner_secret: &PrivateKey, chunk_size: u32, save_path: Option<PathBuf>) -> BuckyResult<(Dir, Vec<(File, PathBuf)>)> {
    if !source.is_dir() {
        return Err(BuckyError::from(BuckyErrorCode::NotMatch));
    }

    let mut all_files_map = HashMap::new();
    let mut entry_deque = VecDeque::new();
    let mut descs = HashMap::new();
    let mut files = Vec::new();
    for entry_ret in walkdir::WalkDir::new(source) {
        match entry_ret {
            Ok(entry) => {
                // 自己就是source的情况，不把parent插入
                if entry.path() != source  {
                    let parent = entry.path().parent().unwrap_or(source);
                    info!("walk {}, parent {}", entry.path().display(), parent.display());
                    if !all_files_map.contains_key(parent) {
                        all_files_map.insert(parent.to_owned(), vec![]);
                    }
                    all_files_map.get_mut(parent).unwrap().push(entry.path().to_owned());
                }

                if entry.file_type().is_dir() {
                    // 如果是个文件夹，插入自己
                    if !all_files_map.contains_key(entry.path()) {
                        all_files_map.insert(entry.path().to_owned(), vec![]);
                    }
                } else if entry.file_type().is_file() {
                    // 这里就把File对象生成出来
                    let file = generate_file_desc(entry.path(), owner_desc, owner_secret, chunk_size, save_path.clone()).await?;
                    descs.insert(entry.path().to_owned(), StandardObject::File(file.clone()));
                    files.push((file, entry.path().to_owned()));
                }
            }
            Err(e) => {
                error!("walk dir {} err {}", source.display(), e);
            }
        }
    }

    // 这里都遍历完成了，把all_files_map的内容输入到deque里
    for (dir, child) in all_files_map {
        entry_deque.push_back((dir, child));
    }

    // 这里先从前往后
    while !entry_deque.is_empty() {
        let (dir, childs) = entry_deque.pop_front().unwrap();
        info!("check dir {}", dir.display());

        // 如果child没有都在desc里，重新插入到队列后
        let mut complete = true;
        for child in &childs {
            if !descs.contains_key(child) {
                complete = false;
                break;
            }
        }

        if !complete {
            info!("insert uncomplete dir {}", dir.display());
            entry_deque.push_back((dir, childs));
            continue;
        }

        info!("process dir {}", dir.display());
        // 检查到所有child都在desc里，创建一个Dir Object
        let mut entrys = HashMap::new();
        let mut bodys = HashMap::new();
        for child in &childs {
            let rel_path = child.strip_prefix(&dir).unwrap().to_string_lossy().to_string();
            // 这里直接从desc拿出来，不会有其他地方用到了
            let obj = descs.remove(child).unwrap();
            let id = obj.calculate_id();
            entrys.insert(rel_path.clone(), InnerNodeInfo::new(Attributes::new(0), InnerNode::ObjId(id.clone())));
            bodys.insert(id, obj.to_vec().unwrap());
        }
        let mut dir_obj = Dir::new(Attributes::new(0), NDNObjectInfo::ObjList(NDNObjectList {
            parent_chunk: None,
            object_map: entrys
        }), bodys).create_time(0).owner(owner_desc.calculate_id()).build();
        // 给dir签名
        let signer = RsaCPUObjectSigner::new(owner_secret.public(), owner_secret.clone());
        sign_and_set_named_object(&signer, &mut dir_obj, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER)).await?;

        // 把dir插入descs
        info!("insert desc {}", dir.display());
        descs.insert(dir.to_owned(), StandardObject::Dir(dir_obj));
    }

    // 到这里，desc里应该只有一个Dir对象
    let dir = descs.remove(source).unwrap();
    if let StandardObject::Dir(dir) = dir {
        Ok((dir, files))
    } else {
        unreachable!()
    }
}

//TODO:计算大文件的hash不容易，FFS FileHash最好能带类型，方便后期扩展FileHash算法
pub async fn generate_file_desc(source: &Path, owner_desc: &StandardObject, owner_secret: &PrivateKey, chunk_size: u32, save_path: Option<PathBuf>) -> BuckyResult<File> {
    info!("generate file desc for {}", source.display());
    let (hash, len) = hash_file(source).await?;

    let chunk_list;
    if len <= chunk_size as u64 {
        //整个file就是一个chunk，直接计算chunkid就好了
        let chunkid = ChunkId::new(&hash, len as u32);
        if let Some(path) = save_path {
            std::fs::copy(source, path.join(chunkid.to_string())).unwrap();
        }
        chunk_list = ChunkList::ChunkInList(vec![chunkid]);
    } else {
        chunk_list = create_chunk_list(source, chunk_size, save_path).await?;
    }
    let mut file = File::new(owner_desc.calculate_id(), len, hash, chunk_list).no_create_time().build();
    debug!("generate desc done");
    let signer = RsaCPUObjectSigner::new(owner_secret.public(), owner_secret.clone());
    sign_and_set_named_object(&signer, &mut file, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER)).await?;
    debug!("sign desc done");

    Ok(file)
}

pub fn write_id_to_file(file_id_file: &Path, fileid: &ObjectId) {
    match std::fs::File::create(file_id_file) {
        Ok(mut file) => {
            if let Err(e) = file.write(fileid.to_string().as_bytes()) {
                warn!("write fileid to {} fail, err {}", file_id_file.display(), e)
            } else {
                info!("fileid write to {}", file_id_file.display());
            }
        },
        Err(e) => {
            warn!("create fileid to {} fail, err {}", file_id_file.display(), e)
        },
    }

}