use cyfs_base::*;
use crate::*;

use std::path::Path;


pub fn get_desc_from_file(desc_path: &Path, secret_path: &Path) -> BuckyResult<(StandardObject, PrivateKey)> {
    debug!("will open secret file: {}", secret_path.display());
    let (secret, _) = PrivateKey::decode_from_file(secret_path, &mut vec![])?;
    debug!("will open desc file: {}", desc_path.display());
    let (desc, _) = StandardObject::decode_from_file(desc_path, &mut vec![])?;
    Ok((desc, secret))
}

pub fn get_device_from_file(desc_path: &Path, secret_path: &Path) -> BuckyResult<(Device, PrivateKey)> {
    let (obj, sec) = get_desc_from_file(desc_path, secret_path)?;
    if let StandardObject::Device(device) = obj {
        Ok((device, sec))
    } else {
        Err(BuckyError::new(BuckyErrorCode::NotMatch, "not device desc"))
    }
}

pub fn get_json_userdata_from_desc(desc: &Device) -> BuckyResult<(serde_json::Value, u64)> {
    let (slice, create_time) = get_userdata_from_desc(desc)?;
    Ok((serde_json::from_slice(slice)?, create_time))
}

pub fn get_userdata_from_desc(desc: &Device) -> BuckyResult<(&[u8], u64)> {
    if let Some(userdata) = desc.body().as_ref().unwrap().user_data() {
        Ok((userdata.as_slice(), desc.body().as_ref().unwrap().update_time()))
    } else {
        return Err(BuckyError::from(BuckyErrorCode::NotFound));
    }
}

// 读取本地的pn配置，在{root}/etc/desc/pn.desc
// 如果函数返回None，就不配置pn
// 如果函数返回Some，就一定将返回值配置成pn
fn load_pn_desc() -> Option<Device> {
    let mut default_pn_file = get_cyfs_root_path();
    default_pn_file.push("etc");
    default_pn_file.push("desc");
    default_pn_file.push("pn.desc");
    if default_pn_file.exists() {
        match  Device::decode_from_file(&default_pn_file, &mut vec![]) {
            Ok((device, _))=> {
                info!("use local config pn server: file={}, id={}", default_pn_file.display(), device.desc().object_id());
                return Some(device);
            }
            Err(e) => {
                error!("invalid pn device: {}, {}", default_pn_file.display(), e);
            }
        }
    }

    None
}


fn load_default_sn_desc() -> Device {
    let mut default_sn_file = get_cyfs_root_path();
    default_sn_file.push("etc");
    default_sn_file.push("desc");
    default_sn_file.push("sn.desc");
    if default_sn_file.exists() {
        match Device::decode_from_file(&default_sn_file, &mut vec![]) {
            Ok((device, _)) =>  {
                info!("use local config sn server: file={}, id={}", default_sn_file.display(), device.desc().object_id());
                return device;
            }
            Err(e) => {
                error!("invalid sn device: {}, {}", default_sn_file.display(), e);
            }
        }
    }

    let sn_raw = match cyfs_base::get_channel() {
        CyfsChannel::Nightly => env!("NIGHTLY_SN_RAW"),
        CyfsChannel::Beta => env!("BETA_SN_RAW"),
        CyfsChannel::Stable => {unreachable!()}
    };
    let (desc, _) = Device::raw_decode(&hex::decode(sn_raw).unwrap()).unwrap();
    desc
}


pub fn get_default_known_peers() -> Vec<Device> {
    let mut ret = vec![];
    let mut default_known_peer_dir = get_cyfs_root_path();
    default_known_peer_dir.push("etc");
    default_known_peer_dir.push("known_peers");
    if default_known_peer_dir.exists() {
        for desc_file in walkdir::WalkDir::new(&default_known_peer_dir).into_iter()
            .filter_map(|e|e.ok()) {
            if desc_file.path().extension().unwrap_or("".as_ref()) == "desc" {
                match Device::decode_from_file(desc_file.path(), &mut vec![]) {
                    Ok((p, _)) => {
                        ret.push(p);
                    }
                    _ => {}
                }
            }
        }
    }

    if ret.len() > 0 {
        return ret;
    }
    /*
    const PUBKEY_HEX: &str = "0030818902818100b2765107d50f440b3f88e9298fe08ed90ff57e177fc111b35f17612628d97f3cef4b9b8ecdf45f99dce9a41a5d3904223d049eeca14d7d0a29d7d33a773a8779fc15899c0a863f7e2ae3e31c527e7e2fbc089fa4e1741e3a82c651a7393494eaee8c13ba5aeea2a747c4f1706ae956a898111e94cf00a6c70087336c6e595ec5020301000100000000000000000000000000000000000000000000";
    const UNIQUE_ID: &str = "637966735f7265706f00000000000000";

    let unique_id = UniqueId::create(&hex::decode(UNIQUE_ID).unwrap());
    let (key, _) = PublicKey::raw_decode(&hex::decode(PUBKEY_HEX).unwrap()).unwrap();

    // <TODO>填写ownerid
    let desc = Device::create(None, unique_id, None, None, Some(13244209973212072), None, vec![
        Endpoint::from_str("W4udp112.74.105.75:8050").unwrap(),
        Endpoint::from_str("W4tcp112.74.105.75:8050").unwrap(),
    ], vec![], None, key, Area::default()).unwrap();
     */

    let (desc, _) = Device::raw_decode(&hex::decode(env!("KNOWN_RAW")).unwrap()).unwrap();
    ret.push(desc);
    ret
}

pub fn get_default_device_desc() -> BuckyResult<(Device, PrivateKey)> {
    let mut desc_path = get_cyfs_root_path();
    desc_path.push("etc");
    desc_path.push("desc");
    get_device_from_file(
        &desc_path.join("device.desc"),
        &desc_path.join("device.sec"),
    )
}

pub fn get_device_desc(name: &str) -> BuckyResult<(Device, PrivateKey)> {
    let mut desc_path = get_cyfs_root_path();
    desc_path.push("etc");
    desc_path.push("desc");
    desc_path.push(name);
    get_device_from_file(
        &desc_path.with_extension("desc"),
        &desc_path.with_extension("sec"),
    )
}


lazy_static::lazy_static!{
    pub static ref DEFAULT_SN: Device = load_default_sn_desc();
    pub static ref DEFAULT_PN: Option<Device> = load_pn_desc();
}

pub fn get_pn_desc() -> Option<Device> {
    DEFAULT_PN.clone()
}

pub fn get_default_sn_desc() -> Device {
    DEFAULT_SN.clone()
}