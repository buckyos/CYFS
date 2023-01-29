use async_std::{
    future, 
    task,
};
use cyfs_base::*;
use cyfs_bdt::{
    *, 
    ndn::{
        channel::{*, protocol::v0::*}
    }, 
};

mod utils;

#[async_std::main]
async fn main() {
    cyfs_util::process::check_cmd_and_exec("bdt-example");
    cyfs_debug::CyfsLoggerBuilder::new_app("bdt-example")
        .level("trace")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("bdt-example", "bdt-example")
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
        let mut params = StackOpenParams::new("bdt-example");
       
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
    

    let src_stack = {
        let mut params = StackOpenParams::new("bdt-example");

        struct RespNotFound {

        }

        #[async_trait::async_trait]
        impl NdnEventHandler for RespNotFound {
            async fn on_newly_interest(
                &self, 
                _stack: &Stack, 
                interest: &Interest, 
                from: &Channel
            ) -> BuckyResult<()> {
                let resp = RespInterest {
                    session_id: interest.session_id.clone(), 
                    chunk: interest.chunk.clone(), 
                    err: BuckyErrorCode::NotFound, 
                    redirect: None,
                    redirect_referer: None,
                    to: None,
                };

                from.resp_interest(resp);
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
        params.ndn_event = Some(Box::new(RespNotFound {}));

        Stack::open(src_dev, src_secret, params).await.unwrap()
    };


    let context = SingleSourceContext::from_desc("".to_owned(), src_stack.local_const().clone());
    let (path, reader) = download_chunk(
        &*down_stack,
        chunkid.clone(), 
        None, 
        context.clone()
    )
    .await.unwrap();

    log::info!("task path: {}", path);

    task::spawn(async move {
        let session = context.wait_session(future::pending::<BuckyError>()).await.unwrap();
        if let DownloadSessionState::Canceled(err) = session.wait_finish().await {
            assert_eq!(err.code(), BuckyErrorCode::NotFound);
        } else {
            unreachable!()
        }
    });

   
    down_store.write_chunk(&chunkid, reader).await.unwrap_err();
}