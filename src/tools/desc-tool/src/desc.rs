use cyfs_base::*;
use log::*;
use std::path::Path;
use std::str::FromStr;

pub fn create_simple_group_desc(
    founder_id: ObjectId,
    admins: Vec<GroupMember>,
    conclusion_limit: Option<u32>,
    area: Area,
) -> Group {
    let area_info = Area::default();
    Group::new_simple_group(founder_id, admins, conclusion_limit, area).build()
}

pub fn create_org_desc(founder_id: ObjectId, area: Area) -> Group {
    let area_info = Area::default();
    Group::new_org(founder_id, area).build()
}

pub fn create_people_desc(
    area: Option<Area>,
    key_bits: usize,
    owner: Option<ObjectId>,
    ood_list: Vec<DeviceId>,
) -> (People, PrivateKey) {
    let area_code = match area {
        Some(v) => v,
        None => {
            warn!("None area, now use default: country: 0, carrier: 0, city: 0, inner: 0");
            Area::default()
        }
    };
    let secret;
    if key_bits < 1024 {
        secret = PrivateKey::generate_secp256k1().unwrap();
    } else {
        secret = PrivateKey::generate_rsa(key_bits).unwrap();
    }
    let pubkey = secret.public();
    (
        People::new(owner, ood_list, pubkey, Some(area_code), None, None).build(),
        secret,
    )
}

pub fn create_device_desc(
    area: Option<Area>,
    category: DeviceCategory,
    key_bits: usize,
    unique_id: &str,
    owner_id: Option<ObjectId>,
    eps: Vec<String>,
    sn_list: Vec<DeviceId>,
    save_path: Option<String>,
) -> Option<(Device, PrivateKey)> {
    let area_code = match area {
        Some(v) => v,
        None => {
            warn!("None area, now use default: country: 0, carrier: 0, city: 0, inner: 0");
            Area::default()
        }
    };

    let mut ep_objs: Vec<Endpoint> = vec![];
    for s in &eps {
        match Endpoint::from_str(s) {
            Ok(ep) => {
                ep_objs.push(ep);
            }
            Err(_e) => {
                error!("ep {} format error", s);
                return None;
            }
        }
    }

    let unique = UniqueId::create(unique_id.as_bytes());

    let secret;
    if key_bits < 1024 {
        secret = PrivateKey::generate_secp256k1().unwrap();
    } else {
        secret = PrivateKey::generate_rsa(key_bits).unwrap();
    }
    let pubkey = secret.public();
    let peer_desc = Device::new(
        owner_id,
        unique,
        ep_objs,
        sn_list,
        vec![],
        pubkey,
        area_code,
        category,
    )
    .build();

    let peer_id = peer_desc.desc().calculate_id();

    if save_path.is_none() {
        return Some((peer_desc, secret));
    }

    let file_base =
        Path::new(save_path.unwrap_or("".to_owned()).as_str()).join(&peer_id.to_string());
    let secret_file_path = file_base.with_extension("sec");
    let desc_file = file_base.with_extension("desc");

    match secret.encode_to_file(secret_file_path.as_ref(), true) {
        Ok(_) => {
            info!("succ encode secret to {}", secret_file_path.display());
        }
        Err(e) => {
            error!("encode secret to file failed, err {}", e);
            return None;
        }
    }

    match peer_desc.encode_to_file(desc_file.as_ref(), true) {
        Ok(_) => {
            info!("success encode peerdesc to {}", desc_file.display());
            Some((peer_desc, secret))
        }
        Err(e) => {
            error!("encode peerdesc to file failed, err {}", e);
            None
        }
    }
}
