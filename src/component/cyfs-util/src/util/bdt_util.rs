use crate::*;
use cyfs_base::*;

use std::path::Path;

pub fn get_desc_from_file(
    desc_path: &Path,
    secret_path: &Path,
) -> BuckyResult<(StandardObject, PrivateKey)> {
    debug!("will open secret file: {}", secret_path.display());
    let (secret, _) = PrivateKey::decode_from_file(secret_path, &mut vec![])?;
    debug!("will open desc file: {}", desc_path.display());
    let (desc, _) = StandardObject::decode_from_file(desc_path, &mut vec![])?;
    Ok((desc, secret))
}

pub fn get_device_from_file(
    desc_path: &Path,
    secret_path: &Path,
) -> BuckyResult<(Device, PrivateKey)> {
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
        Ok((
            userdata.as_slice(),
            desc.body().as_ref().unwrap().update_time(),
        ))
    } else {
        return Err(BuckyError::from(BuckyErrorCode::NotFound));
    }
}

pub(crate) fn load_device_objects_list(root: &Path) -> Vec<(DeviceId, Device)> {
    if !root.is_dir() {
        return vec![];
    }

    let mut loader = DirObjectsSyncLoader::new(root);
    loader.load();

    let objects = loader.into_objects();
    let mut result = Vec::with_capacity(objects.len());
    for (file_path, data) in objects {
        match Device::clone_from_slice(&data) {
            Ok(device) => {
                let id = device.desc().device_id();
                info!(
                    "load local device object: file={}, id={}",
                    file_path.display(),
                    id
                );
                result.push((id, device));
            }
            Err(e) => {
                error!(
                    "invalid local device object: file={}, {}",
                    file_path.display(),
                    e
                );
            }
        }
    }

    result
}

fn load_device_object(file: &Path) -> Vec<(DeviceId, Device)> {
    match Device::decode_from_file(&file, &mut vec![]) {
        Ok((device, _)) => {
            let id = device.desc().device_id();
            info!(
                "load local device object: file={}, id={}",
                file.display(),
                id
            );
            vec![(id, device)]
        }
        Err(e) => {
            error!(
                "invalid local device object: file={}, {}",
                file.display(),
                e
            );
            vec![]
        }
    }
}

// 读取本地的pn配置，在{root}/etc/desc/pn.desc
// 如果函数返回None，就不配置pn
// 如果函数返回Some，就一定将返回值配置成pn
fn load_pn_desc() -> Vec<(DeviceId, Device)> {
    let mut default_pn_file = get_cyfs_root_path();
    default_pn_file.push("etc");
    default_pn_file.push("desc");

    let dir = default_pn_file.join("pn");
    if dir.is_dir() {
        load_device_objects_list(&dir)
    } else {
        default_pn_file.push("pn.desc");
        if default_pn_file.exists() {
            load_device_object(&default_pn_file)
        } else {
            vec![]
        }
    }
}

fn load_default_sn_desc() -> Vec<(DeviceId, Device)> {
    let mut default_sn_file = get_cyfs_root_path();
    default_sn_file.push("etc");
    default_sn_file.push("desc");

    let dir = default_sn_file.join("sn");
    let ret = if dir.is_dir() {
        load_device_objects_list(&dir)
    } else {
        default_sn_file.push("sn.desc");
        if default_sn_file.exists() {
            load_device_object(&default_sn_file)
        } else {
            vec![]
        }
    };

    if ret.len() > 0 {
        return ret;
    }

    let sn_raw = match cyfs_base::get_channel() {
        CyfsChannel::Nightly => env!("NIGHTLY_SN_RAW"),
        CyfsChannel::Beta => env!("BETA_SN_RAW"),
        CyfsChannel::Stable => {
            unreachable!()
        }
    };
    let (desc, _) = Device::raw_decode(&hex::decode(sn_raw).unwrap()).unwrap();
    vec![(desc.desc().device_id(), desc)]
}

pub fn get_default_known_peers() -> Vec<Device> {
    let mut ret = vec![];
    let mut default_known_peer_dir = get_cyfs_root_path();
    default_known_peer_dir.push("etc");
    default_known_peer_dir.push("known_peers");
    if default_known_peer_dir.exists() {
        for desc_file in walkdir::WalkDir::new(&default_known_peer_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
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

lazy_static::lazy_static! {
    pub static ref DEFAULT_SN: Vec<(DeviceId, Device)> = load_default_sn_desc();
    pub static ref DEFAULT_PN: Vec<(DeviceId, Device)> = load_pn_desc();
}

pub fn get_pn_desc() -> &'static Vec<(DeviceId, Device)> {
    &DEFAULT_PN
}

pub fn get_pn_desc_id_list() -> Vec<DeviceId> {
    DEFAULT_PN.iter().map(|item| item.0.clone()).collect()
}

pub fn get_default_sn_desc() -> &'static Vec<(DeviceId, Device)> {
    &DEFAULT_SN
}

pub fn get_default_sn_desc_id_list() -> Vec<DeviceId> {
    DEFAULT_SN.iter().map(|item| item.0.clone()).collect()
}
