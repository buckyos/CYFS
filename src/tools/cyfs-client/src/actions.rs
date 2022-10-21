use std::path::{Path, PathBuf};
use std::io::{Write};
use crate::ffs_client_util::write_id_to_file;
use crate::ffs_client_util;
use crate::meta_helper;
use crate::named_data_client::{NamedCacheClient};

use log::*;
use std::time::Duration;
use cyfs_base::{BuckyResult, File, BuckyError, PrivateKey, NamedObject, ObjectDesc, OwnerObjectDesc, FileEncoder, Dir, BuckyErrorCode, ObjectId, ObjectTypeCode, AnyNamedObject, StandardObject};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use std::str::FromStr;

fn check_path(path: &Path) -> BuckyResult<()> {
    if !path.exists() {
        return Err(BuckyError::new(BuckyErrorCode::NotFound, path.to_str().unwrap().to_string()));
    }
    Ok(())
}

async fn create_file_desc(file_path: &Path, owner_desc: &StandardObject, secret: &PrivateKey, chunk_size:u32, save_path: Option<PathBuf>)->BuckyResult<File> {
    // 1. 读Owner Desc
    check_path(file_path)?;

    let file_desc = ffs_client_util::generate_file_desc(file_path, owner_desc, secret, chunk_size, save_path).await?;
    Ok(file_desc)
}

async fn create_dir_desc(file_path: &Path, owner_desc: &StandardObject, secret: &PrivateKey, chunk_size:u32, save_path: Option<PathBuf>)->BuckyResult<(Dir, Vec<(File, PathBuf)>)> {
    // 1. 读Owner Desc
    check_path(file_path)?;

    let (file_desc, files) = ffs_client_util::generate_dir_desc_2(file_path, owner_desc, secret, chunk_size, save_path).await?;
    Ok((file_desc, files))
}

pub async fn put(client:& mut NamedCacheClient, file:&Path, owner_desc:&StandardObject, secret: &PrivateKey, chunk_size:u32, url_file:Option<PathBuf>, file_id:Option<PathBuf>, save_to_meta: bool)->BuckyResult<(String, Duration)>{
    match client.put_from_file(file, owner_desc, secret, chunk_size, file_id, save_to_meta).await {
        Ok((url, time)) => {
            if let Some(url_file) = url_file {
                if let Ok(mut file) = std::fs::File::create(url_file) {
                    file.write(url.as_bytes()).unwrap();
                    file.flush().unwrap();
                }
            }
            Ok((url, time))
        },
        Err(e) => {
            error!("put err: {}", e);
            Err(e)
        },
    }
}

pub async fn get(client:& NamedCacheClient, url:&str, dest_path:&Path)->BuckyResult<()>{
    match client.get_by_url(url, dest_path).await {
        Ok(_) => {
            info!("get success");
            Ok(())
        },
        Err(e) => {
            error!("get err: {}", e);
            Err(e)
        },
    }
}

// 当id是FileId时，inner_path无效，将File存到dest_path。dest_path必须是文件
// 当id是DirId，将匹配inner_path的内容存储到dest_path下，dest_path必须是目录，暂时不支持Dir嵌套
pub async fn get_by_id(client:& NamedCacheClient, id_str:&str, dest_path:&Path, inner_path: Option<&str>)->BuckyResult<AnyNamedObject>{

    let id = ObjectId::from_str(id_str)?;
    match id.obj_type_code() {
        ObjectTypeCode::File => {
            // 是file，走原来的get file逻辑
            let mut dest_file = async_std::fs::File::create(dest_path).await.unwrap();

            match client.get_file_by_id(id_str, None, &mut dest_file).await {
                Ok(desc) => {
                    info!("get success");
                    Ok(AnyNamedObject::Standard(StandardObject::File(desc)))
                },
                Err(e) => {
                    error!("get err: {}", e);
                    Err(e)
                },
            }
        },
        ObjectTypeCode::Dir => {
            match client.get_dir(id_str, None, inner_path, dest_path).await {
                Ok(desc) => {
                    info!("get success");
                    Ok(AnyNamedObject::Standard(StandardObject::Dir(desc)))
                },
                Err(e) => {
                    error!("get err: {}", e);
                    Err(e)
                },
            }

        },
        _ => {
            error!("onlu support file or dir id!");
            Err(BuckyError::from(BuckyErrorCode::NotSupport))
        }
    }

}

pub async fn create(file:&Path, owner_desc:&StandardObject, secret: &PrivateKey, chunk_size:u32, file_id_path:Option<PathBuf>, save_path: Option<PathBuf>)->BuckyResult<()>{
    if file.is_file() {
        match create_file_desc(file, owner_desc, secret, chunk_size, save_path.clone()).await {
            Ok(file_obj) => {
                let fileid = file_obj.desc().calculate_id();
                if let Some(file_id_path) = file_id_path {
                    write_id_to_file(&file_id_path, &fileid);
                }

                let file_obj_file = match &save_path {
                    Some(save_path)=>{
                        save_path.join(&fileid.to_string()).with_extension("fileobj")
                    },
                    None=>{
                        Path::new(&fileid.to_string()).with_extension("fileobj")
                    }
                };

                if let Err(e) = file_obj.encode_to_file(&file_obj_file, true) {
                    error!("write file obj to {} fail, err {}", file_obj_file.display(), e);
                }
                println!("ffs link: cyfs://{}/{}", file_obj.desc().owner().unwrap(), &fileid);
                Ok(())
            },
            Err(e) => {
                error!("err: {}",e);
                Err(BuckyError::from(e.to_string()))
            }
        }
    } else {
        match create_dir_desc(file, owner_desc, secret, chunk_size, save_path.clone()).await {
            Ok((file_obj, _files)) => {
                let fileid = file_obj.desc().calculate_id();
                if let Some(file_id_path) = file_id_path {
                    write_id_to_file(&file_id_path, &fileid);
                }
                
                let file_obj_file = match &save_path {
                    Some(save_path)=>{
                        save_path.join(&fileid.to_string()).with_extension("fileobj")
                    },
                    None=>{
                        Path::new(&fileid.to_string()).with_extension("fileobj")
                    }
                };

                if let Err(e) = file_obj.encode_to_file(&file_obj_file, true) {
                    error!("write file obj to {} fail, err {}", file_obj_file.display(), e);
                }
                println!("ffs link: cyfs://{}/{}", file_obj.desc().owner().unwrap(), &fileid);
                Ok(())
            },
            Err(e) => {
                error!("err: {}",e);
                Err(BuckyError::from(e.to_string()))
            }
        }
    }
    
}

pub async fn upload(owner_desc:&StandardObject, secret: &PrivateKey, desc:&File, meta_target: Option<String>)->BuckyResult<()>{
    let target = meta_target.map(|s|MetaMinerTarget::from_str(&s).unwrap_or(MetaMinerTarget::default()))
        .unwrap_or(MetaMinerTarget::default());
    let meta_client = MetaClient::new_target(target).with_timeout(std::time::Duration::from_secs(60 * 2));
    let fileid = desc.desc().calculate_id();
    if let Err(e) = meta_helper::create_file_desc_sync(&meta_client, owner_desc, secret, desc).await {
        error!("upload file {} desc failed, err {}", fileid, e);
        return Err(BuckyError::from(e));
    }
    Ok(())
}