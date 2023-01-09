use std::{time::Duration};
use sha2::Digest;
use cyfs_base::*;
use cyfs_util::cache::*;
use cyfs_bdt::*;
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
    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params)
        .await
        .unwrap();



    let mut rn_params = StackOpenParams::new("bdt-example-file-task-uploader");
    let rn_tracker = MemTracker::new();
    let rn_store = TrackedChunkStore::new(NamedDataCache::clone(&rn_tracker), TrackerCache::clone(&rn_tracker));
    rn_params.chunk_store = Some(rn_store.clone_as_reader());
    let rn_stack = Stack::open(rn_dev, rn_secret, rn_params).await.unwrap();


    let mut file_hash = sha2::Sha256::new();
    let mut file_len = 0u64;
    let mut chunkids = vec![];

    let up_dir = cyfs_util::get_named_data_root("bdt-example-file-task-uploader");
    let up_path = up_dir.join(bucky_time_now().to_string());


    {
        let mut up_file = async_std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(up_path.as_path())
            .await
            .unwrap();

        for _ in 0..1024 {
            let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
            let chunk_hash = hash_data(&chunk_data[..]);
            file_hash.input(&chunk_data[..]);
            
            file_len += chunk_len as u64;
            let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
            chunkids.push(chunkid);

            use async_std::io::prelude::*;
            let _ = up_file.write(chunk_data.as_slice()).await.unwrap();
        }

    }   
    

    let file = File::new(
        ObjectId::default(),
        file_len,
        file_hash.result().into(),
        ChunkList::ChunkInList(chunkids),
    )
    .no_create_time()
    .build();


    let _ = rn_store.track_file_in_path(file.clone(), up_path)
        .await
        .unwrap();


    let (path, reader) = download_file(
        &*ln_stack, 
        file, 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![rn_stack.local_const().clone()]), 
    ).await.unwrap();
    let task = ln_stack.ndn().root_task().download().sub_task(path.as_str()).unwrap();

    async_std::task::spawn(async_std::io::copy(reader, async_std::io::sink()));

    loop {
        log::info!("task speed {} progress {}", task.cur_speed(), task.downloaded() as f32 / file_len as f32);
        let _ = async_std::task::sleep(Duration::from_secs(1)).await;
        if let DownloadTaskState::Finished = task.state() {
            break;
        }
    }
    
}
