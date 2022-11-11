use std::io::{Seek, SeekFrom};
use async_std::{fs, io::prelude::*};
use cyfs_base::*;
use cyfs_bdt::{
    download::*, 
    SingleDownloadContext,  
    Stack, 
    StackOpenParams, 
};
use sha2::Digest;
mod utils;


#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-file-task");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-file-task")
        .level("debug")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-file-task", "bdt-example-file-task")
        .exit_on_panic(true)
        .build()
        .start();

    let (ln_dev, ln_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4tcp127.0.0.1:10000"],
    )
    .unwrap();
    let (rn_dev, rn_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4tcp127.0.0.1:10001"],
    )
    .unwrap();

    let mut ln_params = StackOpenParams::new("bdt-example-file-task-downloader");
    ln_params.known_device = Some(vec![rn_dev.clone()]);

    let rn_params = StackOpenParams::new("bdt-example-file-task-uploader");

    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params)
        .await
        .unwrap();

    let rn_stack = Stack::open(rn_dev, rn_secret, rn_params).await.unwrap();

    let mut file_hash = sha2::Sha256::new();
    let mut file_len = 0u64;
    let mut chunkids = vec![];
    let mut chunks = vec![];

    let mut range_hash = sha2::Sha256::new();
    let range = 1000u64..1024u64;

    for _ in 0..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        file_hash.input(&chunk_data[..]);
        range_hash.input(&chunk_data[range.start as usize..range.end as usize]);
        file_len += chunk_len as u64;
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
        chunkids.push(chunkid);
        chunks.push(chunk_data);
    }

    let file = File::new(
        ObjectId::default(),
        file_len,
        file_hash.result().into(),
        ChunkList::ChunkInList(chunkids),
    )
    .no_create_time()
    .build();

    let up_dir = cyfs_util::get_named_data_root("bdt-example-file-task-uploader");
    let up_path = up_dir.join(file.desc().file_id().to_string().as_str());

    {
        let mut up_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(up_path.as_path())
            .await
            .unwrap();

        for chunk in chunks {
            let _ = up_file.write(chunk.as_slice()).await.unwrap();
        }
    }

    let _ = track_file_in_path(&*rn_stack, file.clone(), up_path)
        .await
        .unwrap();

    
    let (_, mut reader) = download_file(
        &*ln_stack, 
        file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();
    {
        let mut hasher = sha2::Sha256::new(); 
        let mut buffer = vec![0u8; (range.end - range.start) as usize];
        reader.seek(SeekFrom::Start(range.start)).unwrap();
        reader.read_exact(&mut buffer[..]).await.unwrap();
        hasher.input(&buffer[..]);

        reader.seek(SeekFrom::Start(1024 * 1024 + range.start)).unwrap();
        reader.read_exact(&mut buffer[..]).await.unwrap();
        hasher.input(&buffer[..]);

        assert_eq!(range_hash.result(), hasher.result());
    }

}
