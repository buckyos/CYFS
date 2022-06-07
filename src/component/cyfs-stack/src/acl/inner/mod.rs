mod default;
mod official;
mod system;

pub(super) use default::*;
pub(super) use official::*;
pub(super) use system::*;

pub(crate) fn get_inner_acl(name: &str) -> Option<&str> {
    let ret = match name {
        "system.app" => APP_ACL_CONFIG,
        "system.perf" => PERF_ACL_CONFIG,

        "official.dsg" => DSG_ACL_CONFIG,
        "official.im" => IM_ACL_CONFIG,
        "official.git" => GIT_ACL_CONFIG,
        "official.drive" => DRIVE_ACL_CONFIG,
        "official.nft" => NFT_ACL_CONFIG,

        _ => {
            return None;
        }
    };

    Some(ret)
}
