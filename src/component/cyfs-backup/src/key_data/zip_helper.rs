use cyfs_base::*;
use super::KeyDataManager;

use std::fs::File;
use std::io::prelude::*;
use std::io::{Seek, Write};
use std::iter::Iterator;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};


pub struct ZipHelper {}

impl ZipHelper {
    pub fn zip_dir_to_buffer(
        src_dir: &Path,
        method: zip::CompressionMethod,
        key_data_manager: &KeyDataManager,
    ) -> BuckyResult<Vec<u8>> {
        let walkdir = WalkDir::new(src_dir);
        let it = walkdir.into_iter();

        let mut buf = Vec::with_capacity(1024 * 10);
        let mut cursor = std::io::Cursor::new(&mut buf);

        Self::zip_dir(
            src_dir,
            &mut it.filter_map(|e| e.ok()),
            src_dir,
            &mut cursor,
            method,
            key_data_manager,
        )?;

        Ok(buf)
    }

    pub fn extract_zip_to_dir(data: impl Read + Seek, target_folder: &Path) -> BuckyResult<()> {
        let mut archive = zip::ZipArchive::new(data)?;
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).map_err(|e| {
                let msg = format!(
                    "get file from zip archive failed: index={}, err={}",
                    index, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })?;

            let fullpath = target_folder.join(file.mangled_name());

            if file.is_dir() {
                debug!("will create dir: {}", fullpath.display());

                std::fs::create_dir_all(&fullpath).map_err(|e| {
                    let msg = format!("create dir error: {}, err={}", fullpath.display(), e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            } else {
                debug!(
                    "will create file: file={}, size={}",
                    fullpath.display(),
                    file.size(),
                );

                if let Some(p) = fullpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(&p).map_err(|e| {
                            let msg = format!("create dir error: {}, err={}", p.display(), e);
                            error!("{}", msg);
                            BuckyError::new(BuckyErrorCode::IoError, msg)
                        })?;
                    }
                }

                let mut outfile = std::fs::File::create(&fullpath).map_err(|e| {
                    let msg = format!("create file error: {}, err={}", fullpath.display(), e);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;

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

            // Get and Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                if let Some(mode) = file.unix_mode() {
                    if let Err(e) =
                        std::fs::set_permissions(&fullpath, std::fs::Permissions::from_mode(mode))
                    {
                        error!(
                            "set file permisson failed! file={}, {}",
                            fullpath.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn path_to_string(path: &std::path::Path) -> String {
        let mut path_str = String::new();
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                if !path_str.is_empty() {
                    path_str.push('/');
                }
                path_str.push_str(&*os_str.to_string_lossy());
            }
        }
        path_str
    }

    fn zip_dir<T>(
        src_dir: &Path,
        it: &mut dyn Iterator<Item = DirEntry>,
        prefix: &Path,
        writer: T,
        method: zip::CompressionMethod,
        key_data_manager: &KeyDataManager,
    ) -> BuckyResult<()>
    where
        T: Write + Seek,
    {
        let mut zip = zip::ZipWriter::new(writer);
        let options = zip::write::FileOptions::default()
            .compression_method(method)
            .unix_permissions(0o755);

        let mut buffer = Vec::new();
        for entry in it {
            let path = entry.path();
            if !key_data_manager.check_filter(path) {
                warn!("key data will be ignored by filter: {}", path.display());
                continue;
            }

            let name = path.strip_prefix(prefix).unwrap();

            // Write file or directory explicitly
            // Some unzip tools unzip files with directory paths correctly, some do not!
            if path.is_file() {
                debug!("adding file {:?} as {:?} ...", path, name);
                zip.start_file(Self::path_to_string(name), options)?;
                let mut f = File::open(path).map_err(|e| {
                    let msg = format!("open local file failed! file={}, {}", path.display(), e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;

                f.read_to_end(&mut buffer).map_err(|e| {
                    let msg = format!(
                        "read local file to buffer failed! file={}, {}",
                        path.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;

                zip.write_all(&*buffer).map_err(|e| {
                    let msg = format!("write buffer to zip failed! file={}, {}", path.display(), e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;

                buffer.clear();
            } else if name.as_os_str().len() != 0 {
                // Only if not root! Avoids path spec / warning
                // and mapname conversion failed error on unzip
                debug!("adding dir {:?} as {:?} ...", path, name);
                zip.add_directory(Self::path_to_string(name), options)
                    .map_err(|e| {
                        let msg = format!("add dir to zip failed! dir={}, {}", name.display(), e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::Failed, msg)
                    })?;
            }
        }

        zip.finish().map_err(|e| {
            let msg = format!(
                "finish write zip file failed! file={}, {}",
                src_dir.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        Ok(())
    }
}
