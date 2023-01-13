use cyfs_base::{CustomObjectId, ObjectId};

pub const STATEPATH_SEPARATOR: &str = "/";

pub const STATEPATH_GROUPS: &str = "groups";

pub const STATEPATH_DEC_STATE: &str = ".dec-state";

pub const STATEPATH_LINK: &str = ".link";
pub const STATEPATH_GROUP_HASH: &str = "group-hash";
pub const STATEPATH_USERS: &str = "users";
pub const STATEPATH_USERS_NONCE: &str = "nonce";
pub const STATEPATH_RANGE: &str = "range";
pub const STATEPATH_BLOCK: &str = "block";

pub const STATEPATH_RPATHS: &str = ".r-paths";

pub const STATEPATH_GROUP_DEC_RPATH: &str = ".update";
pub const STATEPATH_GROUP_DEC_LATEST_VERSION: &str = "latest-version";

lazy_static::lazy_static! {
    pub static ref STATEPATH_GROUP_DEC_ID: ObjectId = ObjectId::from_slice_value(".group".as_bytes());
    pub static ref STATEPATH_GROUP_DEC_ID_STR: String = STATEPATH_GROUP_DEC_ID.to_string();
}

pub struct StatePath {
    group_id: ObjectId,
    group_id_str: String,
    dec_id: ObjectId,
    dec_id_str: String,
    rpath: String,
}

impl StatePath {
    pub fn new(group_id: ObjectId, dec_id: ObjectId, rpath: String) -> Self {
        Self {
            group_id_str: group_id.to_string(),
            group_id,
            dec_id_str: dec_id.to_string(),
            dec_id,
            rpath,
        }
    }

    pub fn join(fields: &[&str]) -> String {
        fields.join(STATEPATH_SEPARATOR)
    }

    pub fn dec_state() -> String {
        STATEPATH_DEC_STATE.to_string()
    }

    pub fn dec_state_group(&self) -> String {
        Self::join(&[STATEPATH_DEC_STATE, self.group_id_str.as_str()])
    }

    pub fn dec_state_dec(&self) -> String {
        Self::join(&[
            STATEPATH_DEC_STATE,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
        ])
    }

    pub fn dec_state_rpath(&self) -> String {
        Self::join(&[
            STATEPATH_DEC_STATE,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
        ])
    }

    pub fn dec_state_rpath_with_sub_path(&self, sub_path: &str) -> String {
        Self::join(&[
            STATEPATH_DEC_STATE,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            sub_path,
        ])
    }

    pub fn link() -> String {
        STATEPATH_LINK.to_string()
    }

    pub fn link_group(&self) -> String {
        Self::join(&[STATEPATH_LINK, self.group_id_str.as_str()])
    }

    pub fn link_dec(&self) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
        ])
    }

    pub fn link_rpath(&self) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
        ])
    }

    pub fn link_group_hash(&self) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            STATEPATH_GROUP_HASH,
        ])
    }

    pub fn link_users(&self) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            STATEPATH_USERS,
        ])
    }

    pub fn link_user(&self, user_id: &ObjectId) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            STATEPATH_USERS,
            user_id.to_string().as_str(),
        ])
    }

    pub fn link_user_nonce(&self, user_id: &ObjectId) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            STATEPATH_USERS,
            user_id.to_string().as_str(),
            STATEPATH_USERS_NONCE,
        ])
    }

    pub fn link_range(&self) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            STATEPATH_RANGE,
        ])
    }

    pub fn link_height(&self, height_seq: u64) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            height_seq.to_string().as_str(),
        ])
    }

    pub fn link_block(&self, height_seq: u64) -> String {
        Self::join(&[
            STATEPATH_LINK,
            self.group_id_str.as_str(),
            self.dec_id_str.as_str(),
            self.rpath.as_str(),
            height_seq.to_string().as_str(),
            STATEPATH_BLOCK,
        ])
    }

    pub fn rpaths(&self) -> String {
        STATEPATH_RPATHS.to_string()
    }
}

pub struct GroupUpdateStatePath;

impl GroupUpdateStatePath {
    pub fn latest_version() -> &'static str {
        STATEPATH_GROUP_DEC_LATEST_VERSION
    }

    pub fn version_seq(version_seq: u64) -> String {
        version_seq.to_string()
    }

    pub fn group_hash(group_hash: &ObjectId) -> String {
        group_hash.to_string()
    }
}
