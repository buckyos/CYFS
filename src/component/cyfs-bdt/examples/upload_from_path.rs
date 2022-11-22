
use std::{
    time::Duration,
    path::PathBuf,
    collections::BTreeMap
};
use async_std::{
    future, 
    io::{prelude::*, Cursor}, 
    task,
    fs::File
};
use cyfs_base::*;
use cyfs_bdt::{
    *, 
    ndn::{
        chunk::*, 
        channel::{*, DownloadSession, protocol::v0::*}
    }, 
};

mod utils;

async fn watch_recv_chunk(stack: StackGuard, chunkid: ChunkId) -> BuckyResult<ChunkId> {
    loop {
        let ret = stack.ndn().chunk_manager().store().get(&chunkid).await;
        if let Ok(mut reader) = ret {
            let mut content = vec![0u8; chunkid.len()];
            let _ = reader.read(content.as_mut_slice()).await?;
            let recv_id = ChunkId::calculate(content.as_slice()).await?;
            return Ok(recv_id);
        } else {
            task::sleep(Duration::from_millis(500)).await;
        }
    }
}

fn watch_resource(task: Box<dyn DownloadTask>) {
    task::spawn(async move {
        loop {
            log::info!("task state: {:?}", task.state());
            task::sleep(Duration::from_millis(500)).await;
        }
    });   
}


#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example-upload-from-path");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-upload-from-path")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-upload-from-path", "bdt-example-upload-from-path")
        .exit_on_panic(true)
        .build()
        .start();
    let (down_dev, down_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10016"],
    )
    .unwrap();

    let (src_dev, src_secret) = utils::create_device(
        "5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR",
        &["W4udp127.0.0.1:10017"],
    )
    .unwrap();


    let (down_stack, down_store) = {
        let mut params = StackOpenParams::new("bdt-example-upload-from-path-down");
       
        let store = MemChunkStore::new();
        params.chunk_store = Some(store.clone_as_reader());
      
        params.known_device = Some(vec![src_dev.clone()]);
        (
            Stack::open(down_dev.clone(), down_secret, params)
                .await
                .unwrap(),
            store
        )
    };


    let (chunk_len, chunk_data) = utils::random_mem(1024, 1024);
    let chunk_hash = hash_data(&chunk_data[..]);
    let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
    
    let dir = cyfs_util::get_named_data_root("bdt-example-upload-from-path-up");
    let mut chunk_pathes = BTreeMap::new();
    {
        let chunk_path = dir.join(chunkid.to_string().as_str());
        let file = File::create(chunk_path.as_path()).await.unwrap();
        let _ = async_std::io::copy(Cursor::new(chunk_data), file).await.unwrap();
        chunk_pathes.insert(chunkid.clone(), chunk_path);
    }

    let src_stack = {
        let mut params = StackOpenParams::new("bdt-example-upload-from-path-up");

        struct UploadFromPath {
            chunk_pathes: BTreeMap<ChunkId, PathBuf>
        }

        #[async_trait::async_trait]
        impl NdnEventHandler for UploadFromPath {
            async fn on_newly_interest(
                &self, 
                stack: &Stack, 
                interest: &Interest, 
                from: &Channel
            ) -> BuckyResult<()> {
                let path = self.chunk_pathes.get(&interest.chunk).cloned().unwrap();
                let cache = FileCache::from_path(path, 0..interest.chunk.len() as u64);
                let _ = start_upload_task_from_cache(stack, interest, from, vec![], cache).await.unwrap();
                Ok(())
            }
        
            fn on_unknown_piece_data(
                &self, 
                _stack: &Stack, 
                _piece: &PieceData, 
                _from: &Channel
            ) -> BuckyResult<DownloadSession> {
                unimplemented!()
            }
        }
        params.ndn_event = Some(Box::new(UploadFromPath {
            chunk_pathes
        }));

        Stack::open(src_dev, src_secret, params).await.unwrap()
    };


    let (_, reader) = download_chunk(
        &*down_stack,
        chunkid.clone(), 
        None, 
        SingleDownloadContext::desc_streams("".to_owned(), vec![src_stack.local_const().clone()]),
    )
    .await.unwrap();
    down_store.write_chunk(&chunkid, reader).await.unwrap();
    let recv = future::timeout(
        Duration::from_secs(50),
        watch_recv_chunk(down_stack.clone(), chunkid.clone()),
    )
    .await
    .unwrap();
    let recv_chunk_id = recv.unwrap();
    assert_eq!(recv_chunk_id, chunkid);

}
