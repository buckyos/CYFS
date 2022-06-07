use log::*;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use zip;

use fs_extra::dir;

use cyfs_base::{
    BuckyError, BuckyResult,
};
use cyfs_util::{get_app_acl_dir, get_app_dir, get_app_web_dir, get_temp_path};
use cyfs_client::NamedCacheClient;
use ood_daemon::get_system_config;

// package 修改自OOD Daemon的ServicePackage，通过source初始化，用于package的下载，解压

pub struct AppPackage {
    name: String,
    fid: String,
    ownerid: String,
    version: String,
}

pub struct AppPath {
    service_dir: PathBuf,
    web_dir: PathBuf,
    acl_dir: PathBuf,
}

impl AppPackage {
    // add code here
    pub fn new(name: &str, fid: &str, ownerid: &str, version: &str) -> Self {
        AppPackage {
            name: name.to_owned(),
            fid: fid.to_owned(),
            ownerid: ownerid.to_owned(),
            version: version.to_owned(),
        }
    }

    //下载并解压到指定目录
    pub async fn download_pkg(&self, client: &NamedCacheClient) -> BuckyResult<AppPath> {
        let acl_path = self.download_permission_config(&client).await?;
        info!("download acl path {}", acl_path.display());
        // 1.下载到temp
        let (file_path, web_path) = self.download(client).await?;
        // 2.解压，将特定target拷贝到app文件夹
        let target_dir = get_app_dir(&self.name);
        // 如果file_path不存在，这个app没有service，不解压
        if file_path.exists() {
            self.extract(&file_path, &target_dir)?;
        }

        let app_path = AppPath {
            service_dir: target_dir,
            web_dir: web_path,
            acl_dir: acl_path,
        };

        Ok(app_path)
    }

    pub async fn download_permission_config(
        &self,
        client: &NamedCacheClient,
    ) -> BuckyResult<PathBuf> {
        let desc_acl_path = get_app_acl_dir(&self.name);
        if !desc_acl_path.exists() {
            std::fs::create_dir_all(&desc_acl_path)?;
        }
        client
            .get_dir(
                &self.fid,
                Some(&self.ownerid),
                Some("acl"),
                desc_acl_path.as_path(),
            )
            .await?;
        // client.get_file_by_id(&self.fid, None, &mut file).await?;
        Ok(desc_acl_path)
    }

    pub async fn download_dep_config(
        &self,
        dest_path: PathBuf,
        client: &NamedCacheClient,
    ) -> BuckyResult<PathBuf> {
        //let dest_dep_path = get_app_dep_dir(&self.name, &self.version);
        if !dest_path.exists() {
            std::fs::create_dir_all(&dest_path)?;
        }
        client
            .get_dir(
                &self.fid,
                Some(&self.ownerid),
                Some("dependent"),
                dest_path.as_path(),
            )
            .await?;
        Ok(dest_path)
    }

    pub async fn install(&self, client: &NamedCacheClient) -> BuckyResult<(PathBuf, PathBuf)> {
        // let acl_path = self.download_permission_config(&client).await?;
        // info!("download acl path {}", acl_path.display());
        // 1.下载到temp
        let (file_path, web_path) = self.download(client).await?;
        // 2.解压，将特定target拷贝到app文件夹
        let target_dir = get_app_dir(&self.name);
        // 如果file_path不存在，这个app没有service，不解压
        if file_path.exists() {
            self.extract(&file_path, &target_dir)?;
        }

        Ok((target_dir, web_path))
    }

    async fn download(&self, client: &NamedCacheClient) -> BuckyResult<(PathBuf, PathBuf)> {
        let target;
        {
            let system_config = get_system_config();
            target = system_config.target.clone();
        }
        let dest_file = get_temp_path().join(&self.fid).with_extension("dl");
        //let mut file = async_std::fs::File::create(&dest_file).await?;
        //拼app service的inner_path，当前为"service/{target}.zip"
        let service_inner_path = format!("service/{}.zip", &target);
        client
            .get_dir(
                &self.fid,
                Some(&self.ownerid),
                Some(&service_inner_path),
                dest_file.as_path(),
            )
            .await?;
        // 将web文件夹下载到/cyfs/app/web/<app_id>
        let desc_web_path = get_app_web_dir(&self.name);
        client
            .get_dir(
                &self.fid,
                Some(&self.ownerid),
                Some("web"),
                desc_web_path.as_path(),
            )
            .await?;
        // client.get_file_by_id(&self.fid, None, &mut file).await?;
        Ok((dest_file, desc_web_path))
    }

    // 提取包内容到目标目录
    pub fn extract(&self, pkg_path: &Path, target_folder: &Path) -> Result<(), BuckyError> {
        // 创建目标目录
        if target_folder.exists() {
            if !target_folder.is_dir() {
                let msg = format!("target exists but not folder: {}", target_folder.display());
                return Err(BuckyError::from(msg));
            } else {
                // FIXME 如果存在目录，并且有内容，是否需要清除？
            }
        } else {
            std::fs::create_dir_all(target_folder)?;
        }

        // 确保临时目录存在
        let tmp_dir = get_temp_path().join(&self.fid);

        if tmp_dir.is_dir() {
            fs::remove_dir_all(&tmp_dir).map_err(|e| {
                error!(
                    "remove tmp_dir failed! path={}, err={}",
                    tmp_dir.display(),
                    e
                );
                e
            })?;
            info!("remove exists tmp_dir success! dir={}", tmp_dir.display());
        }

        // 创建临时目录
        fs::create_dir_all(&tmp_dir).map_err(|e| {
            error!(
                "create tmp_dir failed! path={}, err={}",
                tmp_dir.display(),
                e
            );
            e
        })?;

        // 尝试下载包
        let zip = self.load_pkg(pkg_path)?;

        // 解压到临时目录
        Self::extract_zip(zip, &tmp_dir).map_err(|e| {
            error!(
                "extract zip to tmp_dir error, zip={}, tmp_dir={}, err={}",
                pkg_path.display(),
                tmp_dir.display(),
                e
            );
            e
        })?;

        // 移动目录
        Self::move_dir_contents(&tmp_dir, target_folder).map_err(|e| {
            error!(
                "move from tmp folder to target folder failed! tmp={}, target={}, err={}",
                tmp_dir.display(),
                target_folder.display(),
                e
            );
            e
        })?;
        info!(
            "copy from tmp to target folder success! tmp={}, target={}",
            tmp_dir.display(),
            target_folder.display()
        );

        Ok(())
    }

    fn extract_zip(
        mut archive: zip::ZipArchive<BufReader<fs::File>>,
        target_folder: &Path,
    ) -> Result<(), BuckyError> {
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            #[allow(deprecated)]
            let fullpath = target_folder.join(file.sanitized_name());

            {
                let comment = file.comment();
                if !comment.is_empty() {
                    info!(
                        "package file {} comment: {}",
                        fullpath.as_path().display(),
                        comment
                    );
                }
            }

            if file.is_dir() {
                debug!("will create dir: {}", fullpath.as_path().display());

                if let Err(e) = fs::create_dir_all(&fullpath) {
                    error!("create dir error: {}, err={}", fullpath.display(), e);
                    return Err(BuckyError::from(e));
                }
            } else {
                debug!(
                    "will create file: {}, size={}",
                    fullpath.display(),
                    file.size()
                );

                if let Some(p) = fullpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(&p)?;
                    }
                }

                match fs::File::create(&fullpath) {
                    Ok(mut outfile) => {
                        match std::io::copy(&mut file, &mut outfile) {
                            Ok(count) => {
                                // FIXME 同步文件的修改时间
                                debug!(
                                    "write file complete! {}, count={}",
                                    fullpath.display(),
                                    count
                                );
                            }
                            Err(e) => {
                                error!("write file error: {}, err={}", fullpath.display(), e);
                                return Err(BuckyError::from(e));
                            }
                        }
                    }
                    Err(e) => {
                        error!("create file error: {}, err={}", fullpath.display(), e);
                        return Err(BuckyError::from(e));
                    }
                }
            }

            // Get and Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&fullpath, fs::Permissions::from_mode(mode)).unwrap();
                }
            }
        }

        Ok(())
    }

    fn move_dir_contents(from: &Path, to: &Path) -> Result<u64, BuckyError> {
        let dir_content = dir::get_dir_content(from)
            .map_err(|e| BuckyError::from(format!("get dir err {}", e)))?;
        for directory in dir_content.directories {
            let tmp_to = Path::new(&directory)
                .strip_prefix(from)
                .map_err(|e| BuckyError::from(format!("path strip err {}", e)))?;
            let dir = to.join(&tmp_to);
            if !dir.exists() {
                debug!("will create target dir: {}", dir.display());
                std::fs::create_dir_all(dir)?;
            } else {
                debug!("target dir exists: {}", dir.display());
            }
        }

        let mut result: u64 = 0;
        for file in dir_content.files {
            let tp = Path::new(&file)
                .strip_prefix(from)
                .map_err(|e| BuckyError::from(format!("path strip err: {}", e)))?;
            let path = to.join(&tp);

            debug!("will copy file: {} => {}", file, path.display());

            let mut file_options = fs_extra::file::CopyOptions::new();
            file_options.overwrite = true;

            let mut work = true;
            while work {
                {
                    let result_copy = fs_extra::file::move_file(&file, &path, &file_options);
                    match result_copy {
                        Ok(val) => {
                            result += val;
                            work = false;
                        }
                        Err(err) => {
                            let msg = format!(
                                "move file error! from={}, to={}, err={}",
                                file,
                                path.display(),
                                err
                            );
                            error!("{}", msg);

                            return Err(BuckyError::from(msg));
                        }
                    }
                }
            }
        }

        debug!("will remove tmp dir: {}", from.display());
        if from.exists() {
            std::fs::remove_dir_all(from)?;
        }

        Ok(result)
    }

    fn load_pkg(
        &self,
        pkg_path: &Path,
    ) -> Result<zip::ZipArchive<BufReader<fs::File>>, BuckyError> {
        let file = fs::File::open(pkg_path)?;
        let buf_reader = BufReader::new(file);

        let zip = zip::ZipArchive::new(buf_reader)?;

        Ok(zip)
    }
}
