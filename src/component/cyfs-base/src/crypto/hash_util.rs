use crate::{BuckyError, BuckyResult, HashValue};
use async_std::io::{ErrorKind, ReadExt};
use sha2::Digest;
use std::path::Path;

pub fn hash_data(data: &[u8]) -> HashValue {
    let mut sha256 = sha2::Sha256::new();
    sha256.input(data);
    sha256.result().into()
}

pub async fn hash_stream(reader: &mut (impl ReadExt + Unpin)) -> BuckyResult<(HashValue, u64)> {
    let mut sha256 = sha2::Sha256::new();
    let mut buf = Vec::with_capacity(1024 * 64);
    unsafe {
        buf.set_len(1024 * 64);
    }
    let mut file_len = 0;
    loop {
        match reader.read(&mut buf).await {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                sha256.input(&buf[0..size]);
                file_len = file_len + size;
            }
            Err(e) => {
                if let ErrorKind::Interrupted = e.kind() {
                    continue; // Interrupted
                }
                return Err(BuckyError::from(e));
            }
        }
    }

    Ok((sha256.result().into(), file_len as u64))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn hash_file(path: &Path) -> BuckyResult<(HashValue, u64)> {
    let mut file = async_std::fs::File::open(path).await?;

    let mut sha256 = sha2::Sha256::new();
    let mut buf = Vec::with_capacity(1024 * 64);
    unsafe {
        buf.set_len(1024 * 64);
    }
    let mut file_len = 0;
    loop {
        match file.read(&mut buf).await {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                sha256.input(&buf[0..size]);
                file_len = file_len + size;
            }
            Err(e) => {
                if let ErrorKind::Interrupted = e.kind() {
                    continue; // Interrupted
                }
                return Err(BuckyError::from(e));
            }
        }
    }

    Ok((sha256.result().into(), file_len as u64))
}
