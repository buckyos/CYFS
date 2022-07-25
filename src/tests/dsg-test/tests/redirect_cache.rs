use std::{
    time::Duration, 
    collections::HashMap
};
use async_std::{
    io::Cursor, 
    future
};
use cyfs_base::*;
use cyfs_bdt::{download::download_chunk, ChunkDownloadConfig, ChunkWriter};
use cyfs_lib::*;
use cyfs_dsg_client::*;
use dsg_test::*;




pub fn random_mem(piece: usize, count: usize) -> (usize, Vec<u8>) {
    let mut buffer = vec![0u8; piece * count];
    for i in 0..count {
        let r = rand::random::<u64>();
        buffer[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
    }
    (piece * count, buffer)
}


async fn wait_stored(dsg: &ignore_witness::AllInOneDsg, contract: ObjectId) {
    loop {
        let _ = future::timeout(Duration::from_secs(1), future::pending::<()>()).await;
        let resp = dsg.client().interface().query(DsgQuery::QueryStates { contracts: HashMap::from([(contract.clone(), None)]) }).await.unwrap();
        match resp {
            DsgQuery::RespStates { states } => {
                let state: DsgContractStateObject = dsg.client().interface().get_object_from_noc(states.get(&contract).unwrap().clone()).await.unwrap();
                let state_ref = DsgContractStateObjectRef::from(&state);
                match state_ref.state() {
                    DsgContractState::DataSourceStored => {
                        break;
                    }, 
                    _ => {
                        continue;
                    }
                }
            }, 
            _ => unreachable!()
        }
    }
   
}

#[async_std::test]
async fn download_from_cache_miner() {
    cyfs_debug::CyfsLoggerBuilder::new_app("dsg-all-in-one")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("info"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("dsg-all-in-one", "dsg-all-in-one")
        .build()
        .start();

    let dsg = ignore_witness::AllInOneDsg::new(None, None).await.unwrap();

    let mut chunks = vec![];
    for _ in 0..1 {
        let (chunk_len, chunk_data) = random_mem(1024, 1 * 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunk_id = ChunkId::new(&chunk_hash, chunk_len as u32);
        
        let _ = dsg.stack().ndn_service()
            .put_data(NDNPutDataOutputRequest::new(
                NDNAPILevel::NDC,
                chunk_id.object_id(),
                chunk_len as u64,
                Box::new(Cursor::new(chunk_data)),
            ))
            .await
            .unwrap();
        
        chunks.push(chunk_id);
    }
   

    let contract = DsgContractObjectRef::create(
        dsg.stack(),
        DsgContractDesc {
            data_source: DsgDataSource::Immutable(chunks.clone()),
            storage: DsgStorage::Cache(DsgCacheStorage {
                pub_http: None,
                pub_cyfs: true,
            }),
            miner: dsg.stack().local_device_id().object_id().clone(),
            start_at: bucky_time_now(),
            end_at: bucky_time_now() + Duration::from_secs(100000000).as_micros() as u64,
            witness_dec_id: None,
            witness: DsgNonWitness {},
        },
    )
    .unwrap();
    let contract_ref = DsgContractObjectRef::from(&contract);

    let _ = dsg
        .client()
        .apply_contract(contract_ref.clone())
        .await
        .unwrap();
    let initial_state = contract_ref.initial_state();
    let _ = dsg
        .client()
        .interface()
        .sync_contract_state(&initial_state)
        .await
        .unwrap();

    let _ = future::timeout(Duration::from_secs(30), wait_stored(&dsg, contract_ref.id())).await.unwrap();

    let (requester, store) = local_bdt_stack(dsg.stack(), &["D4tcp0.0.0.0:10000"], None).await.unwrap();

    {
        let chunk = chunks[0].clone();
        let mut config = ChunkDownloadConfig::force_stream(dsg.stack().local_device_id());
        config.referer = Some(BdtDataRefererInfo {
            target: Some(contract_ref.id()), 
            object_id: chunk.object_id().clone(), 
            inner_path: None,
            dec_id: None, 
            req_path: None, 
            referer_object: vec![],
            flags: 0,
        }.encode_string());
        
        let _ = download_chunk(
            &*requester,
            chunk.clone(),
            config, 
            vec![store.clone_as_writer()],
        )
        .await;
        let recv = future::timeout(
            Duration::from_secs(10),
            watch_recv_chunk(requester.clone(), chunk.clone()),
        )
        .await
        .unwrap();
        let recv_chunk_id = recv.unwrap();
        assert_eq!(recv_chunk_id, chunk);
    }
}



#[async_std::test]
async fn download_from_embed_bdt() {
    cyfs_debug::CyfsLoggerBuilder::new_app("dsg-all-in-one")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("info"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("dsg-all-in-one", "dsg-all-in-one")
        .build()
        .start();

    let miner_config = ignore_witness::TestMinerConfig {
        embed_bdt_stack: Some(vec!["D4tcp0.0.0.0:10001".to_owned()])
    };
    let dsg = ignore_witness::AllInOneDsg::new(None, Some(miner_config)).await.unwrap();

    let mut chunks = vec![];
    for _ in 0..2 {
        let (chunk_len, chunk_data) = random_mem(1024, 1 * 1024);
        let chunk_hash = hash_data(&chunk_data[..]);
        let chunk_id = ChunkId::new(&chunk_hash, chunk_len as u32);
        
        let _ = dsg.stack().ndn_service()
            .put_data(NDNPutDataOutputRequest::new(
                NDNAPILevel::NDC,
                chunk_id.object_id(),
                chunk_len as u64,
                Box::new(Cursor::new(chunk_data)),
            ))
            .await
            .unwrap();
        
        chunks.push(chunk_id);
    }
   

    let contract = DsgContractObjectRef::create(
        dsg.stack(),
        DsgContractDesc {
            data_source: DsgDataSource::Immutable(chunks.clone()),
            storage: DsgStorage::Cache(DsgCacheStorage {
                pub_http: None,
                pub_cyfs: true,
            }),
            miner: dsg.stack().local_device_id().object_id().clone(),
            start_at: bucky_time_now(),
            end_at: bucky_time_now() + Duration::from_secs(100000000).as_micros() as u64,
            witness_dec_id: None,
            witness: DsgNonWitness {},
        },
    )
    .unwrap();
    let contract_ref = DsgContractObjectRef::from(&contract);

    let _ = dsg
        .client()
        .apply_contract(contract_ref.clone())
        .await
        .unwrap();
    let initial_state = contract_ref.initial_state();
    let _ = dsg
        .client()
        .interface()
        .sync_contract_state(&initial_state)
        .await
        .unwrap();

    let _ = future::timeout(Duration::from_secs(30), wait_stored(&dsg, contract_ref.id())).await.unwrap();

    let (requester, store) = local_bdt_stack(dsg.stack(), &["D4tcp0.0.0.0:10002"], None).await.unwrap();

    for i in 0..2 {
        let chunk = chunks[i].clone();
        let mut config = ChunkDownloadConfig::force_stream(dsg.stack().local_device_id());
        config.referer = Some(BdtDataRefererInfo {
            target: Some(contract_ref.id()), 
            object_id: chunk.object_id().clone(), 
            inner_path: None,
            dec_id: None, 
            req_path: None, 
            referer_object: vec![],
            flags: 0,
        }.encode_string());
        
        let _ = download_chunk(
            &*requester,
            chunk.clone(),
            config, 
            vec![store.clone_as_writer()],
        )
        .await;
        let recv = future::timeout(
            Duration::from_secs(5),
            watch_recv_chunk(requester.clone(), chunk.clone()),
        )
        .await
        .unwrap();
        let recv_chunk_id = recv.unwrap();
        assert_eq!(recv_chunk_id, chunk);
    }
}
