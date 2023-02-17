use super::generator::*;
use super::loader::*;
use cyfs_base::*;
use cyfs_core::*;

use async_std::io::ReadExt;
use std::collections::HashSet;

async fn test_archive() {
    let file_buffer: Vec<u8> = (0..1024 * 4).map(|_| rand::random::<u8>()).collect();

    let path = cyfs_util::get_temp_path().join("test_archive");
    if !path.is_dir() {
        std::fs::create_dir_all(&path).unwrap();
    }

    let mut objects = HashSet::new();
    let mut generator = ObjectArchiveGenerator::new(
        crate::object_pack::ObjectPackFormat::Zip,
        path.clone(),
        1024 * 1024 * 10,
    );
    for i in 0..1024 * 10 {
        let obj = Text::create(&format!("test{}", i), "", "");
        let id = obj.desc().calculate_id();

        let data = async_std::io::Cursor::new(file_buffer.clone());
        generator.add_data(&id, Box::new(data)).await.unwrap();

        objects.insert(id.clone());
    }

    let mut chunks = HashSet::new();
    for i in 0u32..1024 * 6 {
        let mut file_buffer = file_buffer.clone();
        file_buffer[0] = i.to_be_bytes()[0];
        file_buffer[1] = i.to_be_bytes()[1];
        file_buffer[2] = i.to_be_bytes()[2];
        file_buffer[3] = i.to_be_bytes()[3];

        let chunk_id = ChunkId::calculate_sync(&file_buffer).unwrap();

        let data = async_std::io::Cursor::new(file_buffer.clone());
        generator
            .add_data(&chunk_id.object_id(), Box::new(data))
            .await
            .unwrap();

        chunks.insert(chunk_id);
    }

    let meta = generator.finish().await.unwrap();
    info!("meta: {:?}", meta);

    let mut loader = ObjectArchiveSerializeLoader::load(path).await.unwrap();
    loader.reset_object();
    loop {
        let ret = loader.next_object().await.unwrap();
        if ret.is_none() {
            break;
        }

        let (object_id, mut data) = ret.unwrap();
        assert!(objects.remove(&object_id));

        let mut buf = vec![];
        data.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf, file_buffer);
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
        assert!(chunks.remove(&chunk_id));

        let mut buf = vec![];
        data.read_to_end(&mut buf).await.unwrap();
        let read_chunk_id = ChunkId::calculate_sync(&buf).unwrap();
        assert_eq!(read_chunk_id, chunk_id);
    }

    assert!(chunks.is_empty());
}

#[test]
fn test() {
    async_std::task::block_on(test_archive());
}
