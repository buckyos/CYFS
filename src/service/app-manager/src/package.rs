use log::*;
use std::fs;
use std::path::{Path};

use fs_extra::dir;
use app_manager_lib::RepoMode;

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_util::{get_app_acl_dir, get_app_dep_dir, get_app_dir, get_app_web_dir, get_cyfs_root_path, get_temp_path};
use cyfs_client::NamedCacheClient;
use cyfs_core::DecAppId;
use ood_daemon::get_system_config;
use crate::dapp::DApp;

/**
 AppPackage是无状态的辅助类，用于准备DApp文件，并将其拷贝到指定位置
1.  通过NamedCacheClient下载各文件到tmp目录
2. 将下载的各文件拷贝到指定目录

如果是普通安装，1 -> 2。
如果是本地测试安装，直接走2逻辑

 */

pub struct AppPackage {
}

impl AppPackage {
    pub async fn install(app_id: &DecAppId, dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, repo_mode: RepoMode) -> BuckyResult<()> {
        match repo_mode {
            RepoMode::NamedData => {
                // 先下载到/cyfs/tmp/app/{appid}下
                let tmp_path = get_temp_path().join("app").join(app_id.to_string());
                Self::download(dir, owner, client, &tmp_path).await?;
                Self::install_from_local(app_id, &tmp_path, true)
            }
            RepoMode::Local => {
                let repo_path = get_cyfs_root_path().join("app_repo").join(dir.to_string());
                Self::install_from_local(app_id, &repo_path, false)
            }
        }
    }

    pub async fn download(dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, target_path: &Path) -> BuckyResult<()> {
        // 都下载到/cyfs/tmp/app/{appid}下
        if target_path.exists() {
            fs::remove_dir_all(&target_path)?;
        }
        // 下载acl文件，/cyfs/tmp/app/{appid}/acl
        Self::download_acl(dir, owner, client, &target_path.join("acl")).await?;
        Self::download_dep(dir, owner, client, &target_path.join("dep")).await?;
        let service_path = target_path.join("service");
        // 下载service文件，/cyfs/tmp/app/{appid}/service.dl
        let service_pkg_path = target_path.join("service").with_extension("dl");
        let service_exists = Self::download_service(dir, owner, client, &service_pkg_path).await?;
        if service_exists {
            // 解压service zip文件到tmp目录,/cyfs/tmp/app/{appid}/service
            info!("extract app service {} to {}", service_pkg_path.display(), service_path.display());
            Self::extract(&service_pkg_path, &service_path)?;
        }

        // 下载webdir, /cyfs/tmp/app/{appid}/web
        Self::download_web(dir, owner, client, &target_path.join("web")).await?;
        Ok(())
    }

    pub fn install_from_local(app_id: &DecAppId, local_path: &Path, delete_source: bool) -> BuckyResult<()> {
        info!("install app {} from local path {}", app_id, local_path.display());
        if !local_path.exists() {
            return Err(BuckyError::new(BuckyErrorCode::NotFound, format!("local path {} not found", local_path.display())));
        }
        let app_str = app_id.to_string();
        let service_path = get_app_dir(&app_str);
        let acl_path = get_app_acl_dir(&app_str);
        let dep_path = get_app_dep_dir(&app_str);
        // 拷贝acl
        Self::copy_dir_contents(&local_path.join("acl"), &acl_path)?;
        // 拷贝dep
        Self::copy_dir_contents(&local_path.join("dep"), &dep_path)?;
        // 拷贝service
        Self::copy_dir_contents(&local_path.join("service"), &service_path)?;
        // 拷贝webdir
        let web_path = get_app_web_dir(&app_str);
        Self::copy_dir_contents(&local_path.join("web"), &web_path)?;

        if delete_source {
            let _ = fs::remove_dir_all(local_path);
        }

        // 文件就绪后，尝试load app，并做app的前期准备
        let dapp = DApp::load_from(&service_path)?;
        dapp.prepare()?;

        Ok(())
    }

    pub async fn download_acl(dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, target_path: &Path) -> BuckyResult<bool> {
        Self::download_files(dir, owner, client, "acl", target_path).await.map(|size|size > 0)
    }

    pub async fn download_service(dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, target_path: &Path) -> BuckyResult<bool> {
        let system_config = get_system_config();
        let target = system_config.target.clone();
        //拼app service的inner_path，当前为"service/{target}.zip"
        let service_inner_path = format!("service/{}.zip", &target);
        Self::download_files(dir, owner, client, &service_inner_path, target_path).await.map(|size|size > 0)
    }

    pub async fn download_web(dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, target_path: &Path) -> BuckyResult<bool> {
        // 先尝试下载web.zip文件
        let web_zip_tmp_path = get_temp_path().join(format!("{}-web.zip", dir));
        if Self::download_files(dir, owner, client, "web.zip", &web_zip_tmp_path).await? == 1 {
            // 如果有，这里解压
            info!("extract app web {} to {}", web_zip_tmp_path.display(), target_path.display());
            Self::extract(&web_zip_tmp_path, &target_path)?;
            Ok(true)
        } else {
            Self::download_files(dir, owner, client, "web", target_path).await.map(|size|size > 0)
        }

    }

    pub async fn download_dep(dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, target_path: &Path) -> BuckyResult<bool> {
        Self::download_files(dir, owner, client, "dependent", target_path).await.map(|size|size > 0)
    }

    // 下载一个文件夹
    async fn download_files(dir: &ObjectId, owner: &ObjectId, client: &NamedCacheClient, inner_path: &str, target_path: &Path) -> BuckyResult<usize> {
        let (_, num) = client.get_dir_by_obj(dir, Some(owner.clone()), Some(inner_path), target_path).await.map_err(|e| {
            error!("download {}/{} to {} err {}", dir, inner_path, target_path.display(), e);
            e
        })?;
        if num == 0 {
            info!("not found any file in {}/{}/{}", owner, dir, inner_path);
        } else {
            info!("download {}/{}/{} to {} finish", owner, dir, inner_path, target_path.display());
        }
        info!("download {}/{}/{} to {} finish", owner, dir, inner_path, target_path.display());
        Ok(num)
    }

    // 提取包内容到目标目录
    pub fn extract(pkg_path: &Path, target_folder: &Path) -> BuckyResult<()> {
        if target_folder.is_dir() {
            fs::remove_dir_all(target_folder).map_err(|e| {
                error!("remove target_folder failed! path={}, err={}", target_folder.display(), e);
                e
            })?;
            info!("remove exists target_folder success! dir={}", target_folder.display());
        }

        fs::create_dir_all(target_folder).map_err(|e| {
            error!("create target_folder failed! path={}, err={}", target_folder.display(), e);
            e
        })?;

        let file = fs::File::open(pkg_path)?;

        // 解压到临时目录
        zip_extract::extract(file, target_folder, false).map_err(|e| {
            error!(
                "extract zip to tmp_dir error, zip={}, tmp_dir={}, err={}",
                pkg_path.display(),
                target_folder.display(),
                e
            );
            BuckyError::new(BuckyErrorCode::ZipError, e.to_string())
        })?;
        let _ = fs::remove_file(pkg_path);
        Ok(())
    }

    fn copy_dir_contents(from: &Path, to: &Path) -> BuckyResult<()> {
        if !from.exists() {
            info!("{} not exist, skip copy", from.display());
            return Ok(());
        }
        if !to.exists() {
            info!("dir {} not exist, create", to.display());
            fs::create_dir_all(to)?;
        }
        let mut options = dir::CopyOptions::new();
        options.overwrite = true;
        options.copy_inside = true;
        options.content_only = true;
        dir::copy(from, to, &options).map_err(|e| {
            error!(
                "copy folder error! from={}, to={}, err={}",
                from.display(),
                to.display(),
                e
            );
            BuckyError::new(BuckyErrorCode::IoError, e.to_string())
        })?;

        Ok(())
    }
}
