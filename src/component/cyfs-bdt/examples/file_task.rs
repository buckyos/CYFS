use async_std::{fs, future, io::prelude::*};
use cyfs_base::*;
use cyfs_bdt::{
    download::*, 
    SingleDownloadContext,  
    Stack, 
    StackOpenParams, 
    DownloadTask, 
    DownloadTaskState, 
};
use sha2::Digest;
use std::time::Duration;
mod utils;

async fn watch_task_finish(task: Box<dyn DownloadTask>) -> BuckyResult<()> {
    loop {
        match task.state() {
            DownloadTaskState::Finished => {
                break Ok(());
            }
            _ => {}
        }
    }
}

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

    for _ in 0..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 512);
        let chunk_hash = hash_data(&chunk_data[..]);
        file_hash.input(&chunk_data[..]);
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

    let down_dir = cyfs_util::get_named_data_root("bdt-example-file-task-downloader");
    let down_path = down_dir.join(file.desc().file_id().to_string().as_str());
    let task = download_file_to_path(
        &*ln_stack,
        file,
        SingleDownloadContext::streams(None, vec![rn_stack.local_device_id().clone()]), 
        down_path.as_path(),
    )
    .await
    .unwrap();

    let recv = future::timeout(Duration::from_secs(1), watch_task_finish(task))
        .await
        .unwrap();
    let _ = recv.unwrap();
}
