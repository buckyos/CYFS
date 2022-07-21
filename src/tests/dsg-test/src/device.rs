use std::{
    str::FromStr, 
    time::Duration
};
use async_std::{io::prelude::*, task};
use cyfs_base::*;
use cyfs_util::{
    cache::{NamedDataCache, TrackerCache}
};
use cyfs_bdt::{
    ChunkReader, MemChunkStore, MemTracker, Stack, StackConfig, StackGuard, StackOpenParams,
};
use cyfs_lib::*;


fn create_device(owner: &str, endpoints: &[&str]) -> BuckyResult<(Device, PrivateKey)> {
    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let public_key = private_key.public();
    let owner = ObjectId::from_str(owner)?;
    let ep_list = endpoints
        .iter()
        .map(|ep| Endpoint::from_str(ep).unwrap())
        .collect();
    let device = Device::new(
        Some(owner),
        UniqueId::default(),
        ep_list,
        vec![],
        vec![],
        public_key,
        Area::default(),
        DeviceCategory::PC,
    )
    .build();

    Ok((device, private_key))
}


pub async fn local_bdt_stack(
    stack: &SharedCyfsStack, 
    ep: &[&str],
    config: Option<StackConfig>,
) -> BuckyResult<(StackGuard, MemChunkStore)> {
    let (ln_dev, ln_secret) = create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", ep)?;
    
    let mut ln_params = StackOpenParams::new("");
    let ln_tracker = MemTracker::new();
    let ln_store = MemChunkStore::new(NamedDataCache::clone(&ln_tracker).as_ref());
    ln_params.chunk_store = Some(ln_store.clone_as_reader());
    ln_params.ndc = Some(NamedDataCache::clone(&ln_tracker));
    ln_params.tracker = Some(TrackerCache::clone(&ln_tracker));
    let mut config = config;
    if config.is_some() {
        std::mem::swap(&mut ln_params.config, config.as_mut().unwrap());
    }
    let device = stack.local_device();
    let sn = cyfs_util::get_default_sn_desc();
    ln_params.known_sn = Some(vec![sn]);
    ln_params.known_device = Some(vec![device]);

    let local_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params).await?;

    Ok((local_stack, ln_store))
}

pub async fn watch_recv_chunk(stack: StackGuard, chunkid: ChunkId) -> BuckyResult<ChunkId> {
    loop {
        let ret = stack.ndn().chunk_manager().store().read(&chunkid).await;
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



