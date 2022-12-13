use std::{time::Duration};
use async_std::{future, io::{prelude::*, Cursor}, task};
use cyfs_base::*;
use cyfs_bdt::*;

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

    cyfs_util::process::check_cmd_and_exec("bdt-example-channel");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example-channel")
        .level("trace")
        .console("debug")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example-channel", "bdt-example-channel")
        .exit_on_panic(true)
        .build()
        .start();

    let (ln_dev, ln_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4udp127.0.0.1:10000"]).unwrap();
    let (rn_dev, rn_secret) = utils::create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", &["W4udp127.0.0.1:10001"]).unwrap();
    
    let mut ln_params = StackOpenParams::new("bdt-example-channel-download");
    let ln_store = MemChunkStore::new();
    ln_params.chunk_store = Some(ln_store.clone_as_reader());
    
    ln_params.known_device = Some(vec![rn_dev.clone()]);
    let ln_stack = Stack::open(
        ln_dev.clone(), 
        ln_secret, 
        ln_params).await.unwrap();

    let mut rn_params = StackOpenParams::new("bdt-example-channel-upload");
    rn_params.config.interface.udp.sim_loss_rate = 10;
    let rn_stack = Stack::open(
        rn_dev, 
        rn_secret, 
        rn_params).await.unwrap();
   

    for _ in 1..2 {
        let (chunk_len, chunk_data) = utils::random_mem(1024, 100);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
        
        let dir = cyfs_util::get_named_data_root("bdt-example-channel-upload");
        let path = dir.join(chunkid.to_string().as_str());
        local_chunk_writer(&chunkid, path).write(Cursor::new(chunk_data)).await.unwrap();

        let context = SingleDownloadContext::new("".to_owned());
        context.add_source(DownloadSource {
            target: rn_stack.local_const().clone(), 
            encode_desc: ChunkEncodeDesc::reverse_stream(None, None), 
        });

        let (_, reader) = download_chunk(
            &*ln_stack, 
            chunkid.clone(), 
            None, 
            context, 
        ).await.unwrap();
        ln_store.write_chunk(&chunkid, reader).await.unwrap();
        // watch_resource(task.clone_as_task());

        let recv = future::timeout(Duration::from_secs(50), watch_recv_chunk(ln_stack.clone(), chunkid.clone())).await.unwrap();
        let recv_chunk_id = recv.unwrap();
        assert_eq!(recv_chunk_id, chunkid);
    }

    task::sleep(Duration::from_secs(10000000000)).await;
}



