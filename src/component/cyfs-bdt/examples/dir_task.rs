use async_std::{fs, future, io::prelude::*};
use cyfs_base::*;
use log::debug;
use cyfs_bdt::{
    download::*, ChunkDownloadConfig, DownloadTaskControl, Stack, StackOpenParams, TaskControlState,
};
use sha2::Digest;
#[warn(unused_mut)]
#[warn(unused_imports)]
use std::{collections::HashMap, path::PathBuf, time::Duration};
mod utils;

async fn watch_task_finish(
    task: Box<dyn DownloadTaskControl>,
    dir_task_control: &DirTaskPathControl,
) -> BuckyResult<()> {
    loop {
        match task.control_state() {
            TaskControlState::Finished => {
                debug!("dir task finished, exiting.");
                break Ok(());
            }
            TaskControlState::Downloading(speed, progress) => {
                debug!("watch_task_finish: {}-{}", speed, progress);

                if progress == 100 {
                    let _ = dir_task_control.finish();
                }
            }
            _ => {}
        }

        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

async fn gen_file() -> (PathBuf, File) {
    let mut file_hash = sha2::Sha256::new();
    let mut chunkids = vec![];
    let mut chunks = vec![];

    let (chunk_len, chunk_data) = utils::random_mem(1024, 16 * 1024);
    let chunk_hash = hash_data(&chunk_data[..]);
    file_hash.input(&chunk_data[..]);
    // file_len += chunk_len as u64;
    let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
    chunkids.push(chunkid);
    chunks.push(chunk_data);

    let file = File::new(
        ObjectId::default(),
        chunk_len as u64,
        file_hash.result().into(),
        ChunkList::ChunkInList(chunkids),
    )
    .no_create_time()
    .build();

    let dir = cyfs_util::get_named_data_root("bdt-example-dir-task-uploader");
    let path = dir.join(file.desc().file_id().to_string().as_str());

    {
        let mut up_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(path.as_path())
            .await
            .unwrap();

        for chunk in chunks {
            let _ = up_file.write(chunk.as_slice()).await.unwrap();
        }
    }

    debug!("gen-file: {:#?}", path);

    (path, file)
}

#[async_std::main]
async fn main() {
    // cyfs_util::process::check_cmd_and_exec("bdt-example-file-task");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-file-task")
        .level("debug")
        .console("trace")
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

    // let mut entrys = HashMap::new();
    let mut file_obj_map = HashMap::new();

    let down_dir = cyfs_util::get_named_data_root("bdt-example-dir-task-downloader");
    // let down_path = PathBuf::new();
    let mut down_file_path/* : Vec<_> */ = vec![];
    // let down_path = down_dir.join(file.desc().file_id().to_string().as_str());

    let file_num = {
        if std::env::args().len() >= 2 {
            if let Some(item) = &std::env::args().last() {
                if let Ok(num) = item.parse() {
                    num
                } else {
                    1
                }
            } else {
                1
            }
        } else {
            1
        }
    };

    for _ in 0..file_num {
        let (path, file) = gen_file().await;

        let _ = track_file_in_path(&*rn_stack, file.clone(), path).await;

        let file_id = file.desc().calculate_id();

        file_obj_map.insert(file_id.clone(), file.to_vec().unwrap());

        let down_path = down_dir.join(file_id.to_string().as_str());

        down_file_path.push((file.clone(), down_path));
    }

    let dir = Dir::new(
        Attributes::new(0),
        NDNObjectInfo::ObjList(NDNObjectList {
            parent_chunk: None,
            object_map: HashMap::new(),
        }),
        file_obj_map,
    )
    .create_time(0)
    .build();

    match download_dir_to_path(
        &*ln_stack,
        dir.desc().dir_id(),
        ChunkDownloadConfig::force_stream(rn_stack.local_device_id().clone()),
        down_dir.as_path(),
    ) {
        Ok((task, dir_task_control)) => {
            // dir_task_control.add_file_path(file, path)

            for (file, path) in down_file_path {
                let _ = dir_task_control.add_file_path(file, path.as_path());
            }

            // let _ = dir_task_control.finish();

            let recv = future::timeout(
                Duration::from_secs(1),
                watch_task_finish(task, &dir_task_control),
            )
            .await
            .unwrap();
            let _ = recv.unwrap();
        }
        Err(_) => {}
    };
}
