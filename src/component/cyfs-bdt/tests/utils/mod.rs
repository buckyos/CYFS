use std::str::FromStr;
use cyfs_base::*;
use cyfs_bdt::*;


pub fn random_mem(piece: usize, count: usize) -> (usize, Vec<u8>) {
    let mut buffer = vec![0u8; piece * count];
    for i in 0..count {
        let r = rand::random::<u64>();
        buffer[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
    }
    (piece * count, buffer)
}

pub fn create_device(owner: &str, endpoints: &[&str]) -> BuckyResult<(Device, PrivateKey)> {
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

pub async fn local_stack_pair(
    ln_ep: &[&str],
    rn_ep: &[&str],
) -> BuckyResult<((StackGuard, MemChunkStore), (StackGuard, MemChunkStore))> {
    local_stack_pair_with_config(ln_ep, rn_ep, None, None).await
}

pub async fn local_stack_pair_with_config(
    ln_ep: &[&str],
    rn_ep: &[&str],
    ln_config: Option<StackConfig>,
    rn_config: Option<StackConfig>,
) -> BuckyResult<((StackGuard, MemChunkStore), (StackGuard, MemChunkStore))> {
    let (ln_dev, ln_secret) = create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", ln_ep)?;
    let (rn_dev, rn_secret) = create_device("5aSixgLuJjfrNKn9D4z66TEM6oxL3uNmWCWHk52cJDKR", rn_ep)?;

    let mut ln_params = StackOpenParams::new("");
    let ln_store = MemChunkStore::new();
    ln_params.chunk_store = Some(ln_store.clone_as_reader());
    let mut ln_config = ln_config;
    if ln_config.is_some() {
        std::mem::swap(&mut ln_params.config, ln_config.as_mut().unwrap());
    }

    ln_params.known_device = Some(vec![rn_dev.clone()]);
    let ln_stack = Stack::open(ln_dev.clone(), ln_secret, ln_params).await?;

    let mut rn_params = StackOpenParams::new("");
    let rn_store = MemChunkStore::new();
    rn_params.chunk_store = Some(rn_store.clone_as_reader());
    let mut rn_config = rn_config;
    if rn_config.is_some() {
        std::mem::swap(&mut rn_params.config, rn_config.as_mut().unwrap());
    }

    let rn_stack = Stack::open(rn_dev, rn_secret, rn_params).await?;

    Ok(((ln_stack, ln_store), (rn_stack, rn_store)))
}
