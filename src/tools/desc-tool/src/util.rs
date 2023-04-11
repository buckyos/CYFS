use clap::ArgMatches;
use cyfs_base::{DeviceId, Endpoint, GroupMember, ObjectId, BuckyResult};
use log::*;
use std::convert::TryFrom;
use std::str::FromStr;

pub fn get_objids_from_matches(matches: &ArgMatches, name: &str) -> Option<Vec<ObjectId>> {
    if let Some(strs) = matches.values_of_lossy(name) {
        let mut ret = vec![];
        for str in &strs {
            match ObjectId::from_str(str) {
                Ok(obj) => ret.push(obj),
                Err(_) => {
                    error!("{} not valid objid, ignore", str);
                }
            }
        }
        Some(ret)
    } else {
        None
    }
}

pub fn get_deviceids_from_matches(matches: &ArgMatches, name: &str) -> Option<Vec<DeviceId>> {
    get_objids_from_matches(matches, name).map(|objs| {
        let mut ret = vec![];
        for obj in &objs {
            if let Ok(device_id) = DeviceId::try_from(obj) {
                ret.push(device_id)
            } else {
                error!("id {} is not valid deviceid, ignore", obj);
            }
        }
        ret
    })
}

pub fn get_eps_from_matches(matches: &ArgMatches, name: &str) -> Option<Vec<Endpoint>> {
    if let Some(strs) = matches.values_of_lossy(name) {
        let mut ret = vec![];
        for str in &strs {
            match Endpoint::from_str(str) {
                Ok(obj) => ret.push(obj),
                Err(_) => {
                    error!("{} not valid endpoint, ignore", str);
                }
            }
        }
        Some(ret)
    } else {
        None
    }
}

pub fn get_group_members_from_matches(
    matches: &ArgMatches,
    name: &str,
) -> BuckyResult<Option<Vec<GroupMember>>> {
    if let Some(strs) = matches.values_of_lossy(name) {
        let mut ret = vec![];
        for str in &strs {
            ret.push(GroupMember::from_str(str)?);
        }
        Ok(Some(ret))
    } else {
        Ok(None)
    }
}
