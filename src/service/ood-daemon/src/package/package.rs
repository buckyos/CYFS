use cyfs_base::BuckyError;

use fs_extra::dir;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use zip;

pub struct ServicePackage {
    file: PathBuf,
    tmp_dir: PathBuf,
}

pub fn return_error<T>(msg: &str) -> Result<T, BuckyError> {
    error!("{}", msg);

    return Result::Err(BuckyError::from(msg));
}

impl ServicePackage {
    // add code here
    pub fn new(file: &Path) -> Result<ServicePackage, BuckyError> {
        if !file.exists() || !file.is_file() {
            let msg = format!("package not exists or not invalid file: {}", file.display());
            return return_error(&msg);
        }

        let file_name = file.file_name().unwrap().to_str().unwrap();

        let tmp_dir = cyfs_util::get_temp_path().join(file_name);

        Ok(ServicePackage {
            file: file.to_path_buf(),
            tmp_dir,
        })
    }

    /*
    // 计算包的hash
    pub fn calc_hash(&self) -> Result<String, BuckyError> {
        let mut file = fs::File::open(&self.file)?;

        let mut hasher = Sha256::new();

        std::io::copy(&mut file, &mut hasher)?;

        // read hash digest
        let hex = hasher.result();

        let mut s = String::new();
        for &byte in hex.as_slice() {
            write!(&mut s, "{:X}", byte).expect("Unable to format hex string");
        }

        Ok(s)
    }
    */

    fn udpate_version_file(root: &Path, fid: &str) {
        let version_file = root.join("version");
        if let Err(e) = fs::write(&version_file, fid) {
            error!(
                "create version file error: {} {}",
                version_file.display(),
                e
            );
        }
    }

    // 更新current目录
    pub fn update_current(root: &Path, target_folder: &Path, fid: &str) {
        // fid写入{root}/version文件
        Self::udpate_version_file(root, fid);
        let current_link = root.join("current");

        #[cfg(windows)]
        {
            if current_link.exists() {
                // windows下快捷方式应该只是一个空目录，可以用remove_dir来移除
                if let Err(e) = fs::remove_dir(&current_link) {
                    error!(
                        "remove current link as dir error! link={}, err={}",
                        current_link.display(),
                        e
                    );
                }
            }

            if let Err(e) = std::os::windows::fs::symlink_dir(&target_folder, &current_link) {
                error!(
                    "link target folder to current error! {} => {}, err={}",
                    target_folder.display(),
                    current_link.display(),
                    e
                )
            }
        }

        #[cfg(not(windows))]
        {
            if current_link.exists() {
                // unix下是文件
                if let Err(e) = fs::remove_file(&current_link) {
                    error!(
                        "remove current link as file error! link={}, err={}",
                        current_link.display(),
                        e
                    );
                }
            }

            if let Err(e) = std::os::unix::fs::symlink(&target_folder, &current_link) {
                error!(
                    "link target folder to current error! {} => {}, err={}",
                    target_folder.display(),
                    current_link.display(),
                    e
                )
            }
        }
    }

    // 提取包内容到目标目录
    pub fn extract(&self, target_folder: &Path) -> Result<(), BuckyError> {
        // 创建目标目录
        if target_folder.exists() {
            if !target_folder.is_dir() {
                let msg = format!("target exists but not folder: {}", target_folder.display());
                return return_error(&msg);
            } else {
                // FIXME 如果存在目录，并且有内容，是否需要清除？
            }
        } else {
            std::fs::create_dir_all(target_folder)?;
        }

        // 确保临时目录存在
        if self.tmp_dir.is_dir() {
            if let Err(e) = fs::remove_dir_all(&self.tmp_dir) {
                let msg = format!(
                    "remove tmp_dir failed! path={}, err={}",
                    self.tmp_dir.display(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            } else {
                info!(
                    "remove exists tmp_dir success! dir={}",
                    self.tmp_dir.display()
                );
            }
        }

        // 创建临时目录
        if let Err(e) = fs::create_dir_all(&self.tmp_dir) {
            let msg = format!(
                "create tmp_dir failed! path={}, err={}",
                self.tmp_dir.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        // 尝试加载包
        let zip = self.load_pkg()?;

        // 解压到临时目录
        if let Err(e) = ServicePackage::extract_zip(zip, &self.tmp_dir) {
            error!(
                "extract zip to tmp_dir error, zip={}, tmp_dir={}, err={}",
                self.file.as_path().display(),
                self.tmp_dir.display(),
                e
            );
        }

        // 拷贝对应的target目录到目标目录
        /*
        let target = get_system_config().target.clone();

        let tmp_target_dir = if target.is_empty() {
            self.tmp_dir.clone()
        } else {
            let inner = self.tmp_dir.join(target);
            if !inner.is_dir() {
                let msg = format!(
                    "target dir not found in service package! tmp={}, target={}",
                    self.tmp_dir.display(),
                    inner.display(),
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }

            inner
        };
        */

        // 新版本一个zip包里面就是一个完整service，不再区分target
        let tmp_target_dir= &self.tmp_dir;

        // 移动目录
        let ret = if let Err(e) = Self::move_dir_contents(&tmp_target_dir, target_folder) {
            let msg = format!(
                "move from tmp folder to target folder failed! tmp={}, target={}, err={}",
                tmp_target_dir.display(),
                target_folder.display(),
                e
            );
            error!("{}", msg);

            Err(BuckyError::from(msg))
        } else {
            info!(
                "copy from tmp to target folder success! tmp={}, target={}",
                tmp_target_dir.display(),
                target_folder.display()
            );

            Ok(())
        };

        // 删除临时目录
        if self.tmp_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.tmp_dir) {
                error!("remove pkg tmp dir failed! dir={}, {}", self.tmp_dir.display(), e);
            }
        }
        
        ret
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
                    error!(
                        "create dir error: {}, err={}",
                        fullpath.as_path().display(),
                        e
                    );
                    return Err(BuckyError::from(e));
                }
            } else {
                debug!(
                    "will create file: {}, size={}",
                    fullpath.as_path().display(),
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
                                    fullpath.as_path().display(),
                                    count
                                );
                            }
                            Err(e) => {
                                error!(
                                    "write file error: {}, err={}",
                                    fullpath.as_path().display(),
                                    e
                                );
                                return Err(BuckyError::from(e));
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "create file error: {}, err={}",
                            fullpath.as_path().display(),
                            e
                        );
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
        let dir_content = dir::get_dir_content(from).map_err(|e| {
            let msg = format!("read dir error! dir={}, err={}", from.display(), e);
            BuckyError::from(msg)
        })?;

        for directory in dir_content.directories {
            let tmp_to = Path::new(&directory).strip_prefix(from)?;
            let dir = to.join(&tmp_to);
            if !dir.exists() {
                debug!("will create target dir: {}", dir.display());
                dir::create_all(&dir, false).map_err(|e| {
                    let msg = format!("create dir all error! dir={}, err={}", dir.display(), e);
                    BuckyError::from(msg)
                })?;
            } else {
                debug!("target dir exists: {}", dir.display());
            }
        }

        let mut result: u64 = 0;
        for file in dir_content.files {
            let tp = Path::new(&file).strip_prefix(from)?;
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

        dir::remove(from).map_err(|e| {
            let msg = format!("remove dir error! dir={}, err={}", from.display(), e);
            BuckyError::from(msg)
        })?;

        Ok(result)
    }

    fn load_pkg(&self) -> Result<zip::ZipArchive<BufReader<fs::File>>, BuckyError> {
        let file = fs::File::open(&self.file)?;
        let buf_reader = BufReader::new(file);

        let zip = zip::ZipArchive::new(buf_reader)?;

        Ok(zip)
    }
}

#[cfg(test)]
fn test() {
    let src = r"C:\cyfs\tmp\21f548610000000000a89fa6a7a638a79b501d976173b0d2a41fd245b4d6c8ce\x86_64-pc-windows-msvc";
    let dest = r"C:\cyfs\tmp\21f548610000000000a89fa6a7a638a79b501d976173b0d2a41fd245b4d6c8ce\tmp";

    let mut options = dir::CopyOptions::new();
    options.overwrite = true;
    options.copy_inside = true;

    if let Err(e) = super::ServicePackage::move_dir_contents(&PathBuf::from(src), &PathBuf::from(dest)) {
        error!("move dir error! err={}", e);
    }
}
