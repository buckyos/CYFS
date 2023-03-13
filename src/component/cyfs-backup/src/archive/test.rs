use super::file_meta::*;
use super::generator::*;
use super::index::*;
use super::loader::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

use async_std::io::ReadExt;
use std::collections::HashMap;

fn new_dec(name: &str) -> ObjectId {
    DecApp::generate_id(ObjectId::default(), name)
}

fn gen_meta(i: u64) -> ArchiveInnerFileMeta {
    ArchiveInnerFileMeta {
        access: AccessString::default().value(),
        insert_time: i,
        update_time: i + 1,
        create_dec_id: new_dec(&format!("{}", i)),
        storage_category: NamedObjectStorageCategory::Storage,
        context: Some(format!("context: {}", i)),
    }
}

async fn test_archive() {
    let file_buffer: Vec<u8> = (0..1024 * 4).map(|_| rand::random::<u8>()).collect();

    let path = cyfs_util::get_temp_path().join("test_archive");
    if !path.is_dir() {
        std::fs::create_dir_all(&path).unwrap();
    }

    let mut objects = HashMap::new();
    let mut generator = ObjectArchiveGenerator::new(
        bucky_time_now(),
        crate::object_pack::ObjectPackFormat::Zip,
        ObjectBackupStrategy::Uni,
        path.clone(),
        1024 * 1024 * 10,
    );
    for i in 0..1024 * 10 {
        let obj = Text::create(&format!("test{}", i), "", "");
        let id = obj.desc().calculate_id();

        let meta = gen_meta(i);

        let data = async_std::io::Cursor::new(file_buffer.clone());
        generator
            .add_data(&id, Box::new(data), Some(meta.clone()))
            .await
            .unwrap().unwrap();

        objects.insert(id, meta);
    }

    let mut chunks = HashMap::new();
    for i in 0u32..1024 * 6 {
        let mut file_buffer = file_buffer.clone();
        file_buffer[0] = i.to_be_bytes()[0];
        file_buffer[1] = i.to_be_bytes()[1];
        file_buffer[2] = i.to_be_bytes()[2];
        file_buffer[3] = i.to_be_bytes()[3];

        let chunk_id = ChunkId::calculate_sync(&file_buffer).unwrap();

        let meta = gen_meta(i as u64);
        let data = async_std::io::Cursor::new(file_buffer.clone());
        generator
            .add_data(&chunk_id.object_id(), Box::new(data), Some(meta.clone()))
            .await
            .unwrap().unwrap();

        chunks.insert(chunk_id, meta);
    }

    let meta = generator.finish().await.unwrap();
    info!("meta: {:?}", meta);

    let mut loader = ObjectArchiveSerializeLoader::load(path).await.unwrap();

    let ret = loader.verify().await.unwrap();
    assert!(ret.valid);

    loader.reset_object();
    loop {
        let ret = loader.next_object().await.unwrap();
        if ret.is_none() {
            break;
        }

        let (object_id, mut data) = ret.unwrap();
        let meta = objects.remove(&object_id).unwrap();

        let mut buf = vec![];
        data.data.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, file_buffer);

        // info!("{:?}", meta);
        assert_eq!(meta, data.meta.unwrap());
    }
    assert!(objects.is_empty());

    loader.reset_chunk();
    loop {
        let ret = loader.next_chunk().await.unwrap();
        if ret.is_none() {
            break;
        }

        let (object_id, mut data) = ret.unwrap();
        let chunk_id = ChunkId::try_from(&object_id).unwrap();
        let meta = chunks.remove(&chunk_id).unwrap();

        let mut buf = vec![];
        data.data.read_to_end(&mut buf).await.unwrap();
        let read_chunk_id = ChunkId::calculate_sync(&buf).unwrap();
        assert_eq!(read_chunk_id, chunk_id);

        assert_eq!(meta, data.meta.unwrap());
    }

    assert!(chunks.is_empty());
}

#[test]
fn test() {
    cyfs_base::init_simple_log("test-backup-archive", None);
    async_std::task::block_on(test_archive());
}
