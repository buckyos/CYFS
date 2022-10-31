/*
基于链的自升级组件，基础逻辑如下：
1. 首次启动时，计算/从什么地方读取一个当前的fid
2. 定期从链上读取一个name关联的fid，如果和当前记录的fid不同，则升级
3. 下载这个fid对应的文件到tmp
4. 执行这个tmp下的文件，并退掉自己
5. 在tmp下的文件被执行后，删掉原文件，将自己拷贝到源目录
6. 启动源目录下的文件，并退掉自己
7. 源目录下的文件删掉tmp下的文件
 */

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ChunkList, NamedObject, NameLink, ObjectDesc, ObjectId};
use async_std::stream::StreamExt;
use async_std::io::prelude::*;
use once_cell::sync::OnceCell;
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use cyfs_client::NamedCacheClient;
use log::*;

pub(crate) struct Updater {
    name: String,
    cur_exe: PathBuf,
    fid: OnceCell<ObjectId>,
    meta_client: MetaClient,
    cyfs_client: OnceCell<NamedCacheClient>,
}

// 这里先固定owner id
const OWNER_ID: &str = "5r4MYfF8wo73agKvNjPu7ENuJKABYEFDZ4xi6efweF9D";

// 先不考虑多平台需求，假设monitor运行在单平台上。这里fid只对应一个文件就好了
impl Updater {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            cur_exe: std::env::current_exe().unwrap(),
            fid: OnceCell::new(),
            meta_client: MetaClient::new_target(MetaMinerTarget::default()),
            cyfs_client: OnceCell::new()
        }
    }

    async fn download(&self, id: &ObjectId, dest_path: &Path) -> BuckyResult<()> {
        if self.cyfs_client.get().is_none() {
            let mut client = NamedCacheClient::new();
            client.init(None, None, None).await.map_err(|e| {
                error!("init named cache client err {}", e);
                e
            })?;
            self.cyfs_client.set(client);
        }

        let client = self.cyfs_client.get().unwrap();

        let mut dest_file = async_std::fs::File::create(dest_path).await.map_err(|e| {
            error!("create dest file {} err {}", dest_path.display(), e);
            e
        })?;
        client.get_file_by_id_obj(id, None, &mut dest_file).await.map_err(|e| {
            error!("download file {} to {} err {}", id, dest_path.display(), e);
            e
        })?;
        dest_file.flush().await.map_err(|e| {
            error!("flush file {} err {}", dest_path.display(), e);
            e
        })?;
        Ok(())
    }

    async fn check_update(&self) -> BuckyResult<()> {
        if self.fid.get().is_none() {
            let (hash, len) = cyfs_base::crypto::hash_file(&self.cur_exe).await.map_err(|e| {
                error!("hash current exe {} err {}", self.cur_exe.display(), e);
                e
            })?;
            let fid = cyfs_base::File::new(ObjectId::from_str(OWNER_ID).unwrap(), len, hash, ChunkList::ChunkInList(vec![]))
                .no_create_time().build().desc().calculate_id();
            info!("calc current exe fid {}", &fid);
            self.fid.set(fid);
        }

        let fid = self.fid.get().unwrap();
        let (info, _) = self.meta_client.get_name(&self.name).await?.ok_or(BuckyError::from(BuckyErrorCode::NotFound))?;
        if let NameLink::ObjectLink(obj) = info.record.link {
            if &obj != fid {
                // 下载
                info!("update exe {} => {}", fid, &obj);
                let exe_name = self.cur_exe.file_name().unwrap();
                let dest_path = std::env::temp_dir().join(exe_name);
                self.download(&obj, &dest_path).await?;

                // 启动tmp path的新文件，配置环境变量为本exe的地址
                let mut cmd = std::process::Command::new(&dest_path);
                ::cyfs_util::ProcessUtil::detach(&mut cmd);
                let child = cmd
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .env("UPDATE_EXE_PATH", &self.cur_exe)
                    .spawn().map_err(|e| {
                        error!("spawn tmp exe {} err {}", dest_path.display(), e);
                        e
                    })?;
                info!("download and exec tmp exe {}, pid {}. exit", dest_path.display(), child.id());
                std::process::exit(0)
            }
        }

        Ok(())
    }

    fn update_src_exe(&self, org_exe_path: &Path) -> BuckyResult<()> {
        let mut success = false;
        for _ in 0..2 {
            match std::fs::remove_file(&org_exe_path) {
                Ok(_) => {
                    success = true;
                    break;
                }
                Err(e) => {
                    error!("delete src file {} err {}", org_exe_path.display(), e);
                }
            }
            // 因为是升级操作，这里直接thread sleep
            std::thread::sleep(Duration::from_secs(3));
        }
        if !success {
            error!("update failed, delete src exe {} failed", org_exe_path.display());
            return Err(BuckyError::from(BuckyErrorCode::AlreadyExists));
        }
        // 拷贝自己到原来的位置
        std::fs::copy(&self.cur_exe, &org_exe_path).map_err(|e|{
            error!("update failed, copy new exe {} => {} failed, err {}", self.cur_exe.display(), org_exe_path.display(), e);
            e
        })?;
        // 启动原目录下的文件
        let mut cmd = std::process::Command::new(&org_exe_path);
        ::cyfs_util::ProcessUtil::detach(&mut cmd);
        cmd.env_remove("UPDATE_EXE_PATH")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn().map_err(|e|{
            error!("update failed, start new exe {} failed, err {}", org_exe_path.display(), e);
            e
        })?;

        // 退出自己
        std::process::exit(0)
    }

    pub fn start(self) -> BuckyResult<()> {
        let exe_name = self.cur_exe.file_name().ok_or(BuckyError::from(BuckyErrorCode::InvalidParam))?;
        let tmp_dir = std::env::temp_dir();
        // 如果这个可执行文件在tmp path，这是升级的一环
        if let Ok(src) = std::env::var("UPDATE_EXE_PATH") {
            info!("run in update mode, src exe path {}", &src);
            self.update_src_exe(Path::new(&src));
        } else {
            info!("run in normal mode");
            let tmp_exe_path = tmp_dir.join(exe_name);
            if tmp_exe_path.exists() {
                for _ in 0..2 {
                    if let Ok(_) = std::fs::remove_file(&tmp_exe_path) {
                        info!("delete tmp exe {}", tmp_exe_path.display());
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(3));
                }
            }
        }

        // 启动一个timer，30分钟检查一次链
        let arc_self = std::sync::Arc::new(self);
        async_std::task::spawn(async move {
            arc_self.check_update().await;
            let mut interval = async_std::stream::interval(Duration::from_secs(30*60));
            while let Some(_) = interval.next().await {
                arc_self.check_update().await;
            }
        });

        Ok(())

    }
}