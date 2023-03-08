use super::zip_helper::*;
use cyfs_base::*;
use cyfs_util::get_cyfs_root_path;

#[test]
fn test() {
    use std::io::Write;
    let mut buf = vec![];
    let mut cursor = std::io::Cursor::new(&mut buf);
    cursor.write_all("xxxx".as_bytes()).unwrap();
}

#[test]
fn test_zip() {
    let root = get_cyfs_root_path().join("etc");
    let buf = ZipHelper::zip_dir_to_buffer(&root, zip::CompressionMethod::Stored).unwrap();

    let data = std::io::Cursor::new(buf);
    let target = get_cyfs_root_path().join("tmp/etc");
    ZipHelper::extract_zip_to_dir(data, &target).unwrap();
}