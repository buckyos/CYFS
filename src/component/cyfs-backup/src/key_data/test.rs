use super::{zip_helper::*, KeyDataManager};
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
    cyfs_base::init_simple_log("test-key-data-backup", None);

    let root = get_cyfs_root_path().join("etc");

    let filter_dir = get_cyfs_root_path().join("etc").join("gateway\\**");
    let filters =  vec![
        filter_dir.as_os_str().to_string_lossy().to_string(),
    ];

    info!("filters: {:?}", filters);

    let key_data_manager = KeyDataManager::new_uni("", &filters).unwrap();
    let buf = ZipHelper::zip_dir_to_buffer(&root, zip::CompressionMethod::Stored, &key_data_manager).unwrap();

    let data = std::io::Cursor::new(buf);
    let target = get_cyfs_root_path().join("tmp/etc");
    ZipHelper::extract_zip_to_dir(data, &target).unwrap();
}