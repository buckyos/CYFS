use super::pack::*;
use cyfs_base::*;
use cyfs_core::*;

use std::collections::HashSet;

async fn test_pack() {
    let count: usize = 1024 * 10;
    let file_buffer: Vec<u8> = (0..1024 * 4).map(|_| rand::random::<u8>()).collect();

    let aes_key = AesKey::random();

    let path = cyfs_util::get_temp_path().join("test_pack");
    if !path.is_dir() {
        std::fs::create_dir_all(&path).unwrap();
    }

    let backup_file = path.join("backup.zip");

    let mut pack = ObjectPackFactory::create_zip_writer(backup_file.clone(), Some(aes_key.clone()));
    pack.open().await.unwrap();

    for i in 0..count {
        let obj = Text::create(&format!("test{}", i), "", "");
        let id = obj.desc().calculate_id();

        if i % 2 == 0 {
            let data = async_std::io::Cursor::new(file_buffer.clone());
            pack.add_data(&id, Box::new(data), Some(id.to_vec().unwrap())).await.unwrap().unwrap();
        } else {
            pack.add_data_buf(&id, &file_buffer, Some(id.to_vec().unwrap())).await.unwrap().unwrap();
        }
        
        if i % 1024 == 0 {
            info!("gen dir index: {}", i);
            // async_std::task::sleep(std::time::Duration::from_secs(5)).await;

            let len = pack.flush().await.unwrap();
            info!("pack file len: {}", len);
        }
    }

    pack.finish().await.unwrap();

    let mut pack_reader = ObjectPackFactory::create_zip_reader(backup_file, Some(aes_key.clone()));
    pack_reader.open().await.unwrap();

    let mut all = HashSet::new();
    for i in 0..count {
        let obj = Text::create(&format!("test{}", i), "", "");
        let id = obj.desc().calculate_id();

        all.insert(id.clone());

        info!("will get data {}", i);
        let data = pack_reader.get_data(&id).await.unwrap().unwrap();
        let buf = data.data.into_buffer().await.unwrap();
        assert_eq!(buf, file_buffer);

        let data_id = ObjectId::clone_from_slice(data.meta.as_ref().unwrap()).unwrap();
        assert_eq!(data_id, id);
    }

    pack_reader.reset().await;
    loop {
        let ret = pack_reader.next_data().await.unwrap();
        if ret.is_none() {
            break;
        }

        let (object_id, data) = ret.unwrap();
        assert!(all.remove(&object_id));

        let buf = data.data.into_buffer().await.unwrap();
        assert_eq!(buf, file_buffer);

        let data_id = ObjectId::clone_from_slice(data.meta.as_ref().unwrap()).unwrap();
        assert_eq!(data_id, object_id);
    }

    assert!(all.is_empty());
}

#[test]
fn test() {
    cyfs_util::init_log("test-backup-object-pack", Some("debug"));
    async_std::task::block_on(test_pack());
}