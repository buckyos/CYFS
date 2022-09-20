use std::{
    time::Duration, 
    collections::HashMap
};
use async_std::{
    io::Cursor, 
    future
};
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_dsg_client::*;
use dsg_service::*;
use dsg_test::*;


pub fn random_mem(piece: usize, count: usize) -> (usize, Vec<u8>) {
    let mut buffer = vec![0u8; piece * count];
    for i in 0..count {
        let r = rand::random::<u64>();
        buffer[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
    }
    (piece * count, buffer)
}


async fn query_state(dsg: &ignore_witness::AllInOneDsg, contract: ObjectId) -> DsgContractStateObject {
    let resp = dsg.client().interface().query(DsgQuery::QueryStates { contracts: HashMap::from([(contract.clone(), None)]) }).await.unwrap();
    match resp {
        DsgQuery::RespStates { states } => dsg.client().interface().get_object_from_noc(states.get(&contract).unwrap().clone()).await.unwrap(), 
        _ => unreachable!()
    }
}

async fn wait_stored(dsg: &ignore_witness::AllInOneDsg, contract: ObjectId) {
    loop {
        let _ = future::timeout(Duration::from_secs(1), future::pending::<()>()).await;
        let state = query_state(dsg, contract).await;
        let state_ref = DsgContractStateObjectRef::from(&state);
        match state_ref.state() {
            DsgContractState::DataSourceStored => {
                break;
            }, 
            _ => {
                continue;
            }
        }
    }
}


async fn wait_broken(dsg: &ignore_witness::AllInOneDsg, contract: ObjectId) {
    loop {
        let _ = future::timeout(Duration::from_secs(1), future::pending::<()>()).await;
        let state = query_state(dsg, contract).await;
        let state_ref = DsgContractStateObjectRef::from(&state);
        match state_ref.state() {
            DsgContractState::ContractBroken => {
                break;
            }, 
            _ => {
                continue;
            }
        }
    }
}

#[async_std::test]
async fn break_contract() {
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

    let mut service_config = DsgServiceConfig::default();
    service_config.atomic_interval = Duration::from_secs(10);
    service_config.challenge_interval = Duration::from_secs(10);
    service_config.store_challenge.live_time = Duration::from_secs(10);
    let dsg = ignore_witness::AllInOneDsg::new(Some(service_config), None).await.unwrap();

    let (chunk_len, chunk_data) = random_mem(1024, 16 * 1024);
    let chunk_hash = hash_data(&chunk_data[..]);
    let chunk_id = ChunkId::new(&chunk_hash, chunk_len as u32);

    let _ = dsg
        .stack()
        .ndn_service()
        .put_data(NDNPutDataOutputRequest::new(
            NDNAPILevel::NDC,
            chunk_id.object_id(),
            chunk_len as u64,
            Box::new(Cursor::new(chunk_data)),
        ))
        .await
        .unwrap();

    let contract = DsgContractObjectRef::create(
        dsg.stack(),
        DsgContractDesc {
            data_source: DsgDataSource::Immutable(vec![chunk_id]),
            storage: DsgStorage::Backup(DsgBackupStorage::new()),
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
    
    let state = query_state(&dsg, contract_ref.id()).await;

    let state_ref = DsgContractStateObjectRef::from(&state);
    match state_ref.state() {
        DsgContractState::DataSourceStored => {
            let prepared_state = dsg.client().interface().get_object_from_noc(state_ref.prev_state_id().unwrap().clone()).await.unwrap();
            let prepared_ref = DsgContractStateObjectRef::from(&prepared_state);
            match prepared_ref.state() {
                DsgContractState::DataSourcePrepared(prepared) => {
                    for chunk in &prepared.chunks {
                        // let _ = dsg.stack().ndn_service().delete_data(NDNDeleteDataOutputRequest::new(NDNAPILevel::NDC, chunk.object_id(), None)).await.unwrap();
                        let _ = dsg.stack().ndn_service().delete_data(NDNDeleteDataOutputRequest::new(NDNAPILevel::NDC, chunk.object_id(), None)).await;
                    }
                }, 
                _ => unreachable!()
            }
        }, 
        _ => unreachable!()
    }

    let _ = future::timeout(Duration::from_secs(30), wait_broken(&dsg, contract_ref.id())).await.unwrap();

    
}