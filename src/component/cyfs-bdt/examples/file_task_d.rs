use std::{
    sync::Arc,
    time::Duration, str::FromStr, 
    env,
};
use async_std::{
    future, 
    // fs, 
    // io::prelude::*, 
    task
};
// use sha2::Digest;
use cyfs_base::*;
use cyfs_bdt::{
    Stack, 
    StackOpenParams, 
    DownloadTaskControl, 
    TaskControlState, 
    SingleDownloadContext, 
    download::*,
    ndn::ChunkWriter,
};
mod utils;

async fn watch_task_finish(task: Box<dyn DownloadTaskControl>) -> BuckyResult<()> {
    loop {
        match task.control_state() {
            TaskControlState::Finished(_) => {
                // log::info!("file task finish with avg speed {}", speed);
                break Ok(());
            },
            _ => {}
        }
    }
}

#[derive(Clone)]
struct TestWriter(Arc<()>);

impl TestWriter {
    fn new() -> TestWriter {
        Self(Arc::new(()))
    }
}

impl std::fmt::Display for TestWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }

}

#[async_trait::async_trait]
impl ChunkWriter for TestWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    // 写入一组chunk到文件
    async fn write(&self, _chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        println!("{}", content.len());
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn err(&self, _err: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }

}

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-file-task-downloader");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-file-task-downloader")
        .level("debug")
        .console("trace")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-file-task-downloader", "bdt-example-file-task-downloader")
        .exit_on_panic(true)
        .build()
        .start();
    
    let (ln_dev, ln_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4tcp127.0.0.1:20000"]).unwrap();

    let (rn_redirect, _) = utils::create_device_v2("redirect", "", false).unwrap();
    let (rn_dev, _) = utils::create_device_v2("normal", "", false).unwrap();
    
    let ln_dev_id = ln_dev.desc().device_id();
    println!("ln_dev_id={}", ln_dev_id);

    let mut ln_params = StackOpenParams::new("bdt-example-file-task-downloader");
    ln_params.known_device = Some(vec![rn_dev.clone(), rn_redirect.clone()]);
    
    // let rn_params = StackOpenParams::new("bdt-example-file-task-uploader");

    let ln_stack = Stack::open(
        ln_dev.clone(), 
        ln_secret, 
        ln_params).await.unwrap();

    // let rn_stack = Stack::open(
    //     rn_dev, 
    //     rn_secret, 
    //     rn_params).await.unwrap();
    
    let _down_dir = cyfs_util::get_named_data_root("bdt-example-file-task-downloader-downloader");
    // let down_path = down_dir.join("test-rediect-downloader.tmp");

    {
        let chunk = {
            let args = env::args().collect::<Vec<String>>();

            if args.len() >= 2 {
                ChunkId::from_str(args.get(1).unwrap())
            } else {
                unreachable!()
            }
        }.unwrap();
        println!("download {}.", chunk);
        // let chunk = ChunkId::from_str("7C8WUvq215QXhufKkTTfhk5JTCQAqmLkhe2B6LCAdA2t").unwrap();

        let writer = TestWriter::new();


        let task = 
            download_chunk(&*ln_stack, 
                        chunk, 
                        SingleDownloadContext::streams(None, vec![rn_dev.desc().device_id().clone()]), 
                        vec![Box::new(writer)]).await.unwrap();


        let _ = future::timeout(Duration::from_secs(1), watch_task_finish(task)).await.unwrap();
    }
    println!("redirect test finished.");

    {
        let chunk = {
            let args = env::args().collect::<Vec<String>>();

            if args.len() >= 3 {
                ChunkId::from_str(args.get(2).unwrap())
            } else {
                return;
            }
        }.unwrap();
        println!("download {}.", chunk);

        // let chunk = ChunkId::from_str("7C8WUvq215QXhufKkTTfhk5JTCQAqmLkhe2B6LCAdA2t").unwrap();

        let writer = TestWriter::new();


        let task = 
            download_chunk(&*ln_stack, 
                        chunk, 
                        SingleDownloadContext::streams(None, vec![rn_dev.desc().device_id().clone()]), 
                        vec![Box::new(writer)]).await.unwrap();

        let _ = future::timeout(Duration::from_secs(1), watch_task_finish(task)).await.unwrap();
    }

    task::sleep(Duration::from_secs(10000000000)).await;
}



