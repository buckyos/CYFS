use std::{str::FromStr, time::Duration};
use async_std::io::Cursor;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_dsg_client::*;
use dsg_test::ignore_witness::AllInOneConsumer;




pub fn random_mem(piece: usize, count: usize) -> (usize, Vec<u8>) {
    let mut buffer = vec![0u8; piece * count];
    for i in 0..count {
        let r = rand::random::<u64>();
        buffer[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
    }
    (piece * count, buffer)
}

#[async_std::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let miner = ObjectId::from_str(args[1].as_str()).unwrap();
    cyfs_debug::CyfsLoggerBuilder::new_app("dsg-all-in-one")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("off"), Some("off"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("dsg-all-in-one", "dsg-all-in-one")
        .build()
        .start();

    let dsg = AllInOneConsumer::new(miner.clone()).await.unwrap();

    let mut chunks = vec![];
    {
        let (chunk_len, chunk_data) = random_mem(1024, 4096);
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
    {
        let (chunk_len, chunk_data) = random_mem(1024, 4096);
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
    {
        let (part_len, part_data) = random_mem(1024, 2048);
        let chunk_len = part_len + 199;
        let mut chunk_data = vec![0u8; chunk_len];
        chunk_data[..part_len].copy_from_slice(&part_data[..]);
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
            data_source: DsgDataSource::Immutable(chunks),
            storage: DsgStorage::Cache(DsgCacheStorage {
                pub_http: None,
                pub_cyfs: false,
            }),
            miner,
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

    async_std::task::block_on(async_std::future::pending::<()>());
}
