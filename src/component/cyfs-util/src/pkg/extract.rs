use cyfs_base::BuckyResult;

use std::path::{Path, PathBuf};
use std::{fs, io};
use zip::ZipArchive;

pub fn extract_from_zip(zip_path: &Path, dest_path: &Path) -> BuckyResult<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let mut file_path = PathBuf::from(dest_path);

        #[allow(deprecated)]
        file_path.push(file.sanitized_name());
        if file.is_dir() {
            fs::create_dir_all(&file_path).unwrap_or(());
        } else {
            if let Some(path) = file_path.parent() {
                ensure_dir(path);
            }

            let mut out = fs::File::create(&file_path)?;
            io::copy(&mut file, &mut out)?;
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&file_path, fs::Permissions::from_mode(mode)).unwrap();
            }
        }
    }

    return Ok(());
}

fn ensure_dir<P: AsRef<Path>>(path: P) {
    if !path.as_ref().exists() {
        fs::create_dir_all(path).unwrap_or(());
    }
}
