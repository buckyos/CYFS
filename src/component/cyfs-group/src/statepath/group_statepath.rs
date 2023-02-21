use cyfs_base::{ObjectId, ObjectIdDataBuilder};

pub const STATE_PATH_SEPARATOR: &str = "/";
pub const GROUP_STATE_PATH_DEC_STATE: &str = ".dec-state";
pub const GROUP_STATE_PATH_LINK: &str = ".link";
pub const GROUP_STATE_PATH_GROUP_BLOB: &str = "group-blob";
pub const GROUP_STATE_PATH_LAST_VOTE_ROUNDS: &str = "last-vote-round";
pub const GROUP_STATE_PATH_LAST_QC: &str = "last-qc";
pub const GROUP_STATE_PATH_RANGE: &str = "range";
pub const GROUP_STATE_PATH_PREPARES: &str = "prepares";
pub const GROUP_STATE_PATH_PRE_COMMITS: &str = "pre-commits";
pub const GROUP_STATE_PATH_FINISH_PROPOSALS: &str = "finish-proposals";
pub const GROUP_STATE_PATH_FLIP_TIME: &str = "flip-time";
pub const GROUP_STATE_PATH_RECYCLE: &str = "recycle";
pub const GROUP_STATE_PATH_ADDING: &str = "adding";

pub const STATEPATH_GROUP_DEC_RPATH: &str = ".update";
pub const STATEPATH_GROUP_DEC_LATEST_VERSION: &str = "latest-version";

lazy_static::lazy_static! {
    pub static ref STATEPATH_GROUP_DEC_ID: ObjectId = ObjectIdDataBuilder::new().data(".group".as_bytes()).build().unwrap();
    pub static ref STATEPATH_GROUP_DEC_ID_STR: String = STATEPATH_GROUP_DEC_ID.to_string();
}

pub struct GroupStatePath {
    rpath: String,
    root: String,
    dec_state: String,
    link: String,
    group_blob: String,
    last_vote_round: String,
    last_qc: String,
    range: String,
    prepares: String,
    pre_commits: String,
    finish_proposals: String,
    flip_time: String,
    recycle: String,
    adding: String,
}

impl GroupStatePath {
    pub fn new(rpath: String) -> Self {
        Self {
            root: Self::join(&["", rpath.as_str()]),
            dec_state: Self::join(&["", rpath.as_str(), GROUP_STATE_PATH_DEC_STATE]),
            link: Self::join(&["", rpath.as_str(), GROUP_STATE_PATH_LINK]),
            group_blob: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_GROUP_BLOB,
            ]),
            last_vote_round: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_LAST_VOTE_ROUNDS,
            ]),
            last_qc: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_LAST_QC,
            ]),
            range: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_RANGE,
            ]),
            prepares: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_PREPARES,
            ]),
            pre_commits: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_PRE_COMMITS,
            ]),
            finish_proposals: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_FINISH_PROPOSALS,
            ]),
            flip_time: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_FINISH_PROPOSALS,
                GROUP_STATE_PATH_FLIP_TIME,
            ]),
            recycle: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_FINISH_PROPOSALS,
                GROUP_STATE_PATH_RECYCLE,
            ]),
            adding: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_FINISH_PROPOSALS,
                GROUP_STATE_PATH_ADDING,
            ]),
            rpath,
        }
    }

    pub fn join(fields: &[&str]) -> String {
        fields.join(STATE_PATH_SEPARATOR)
    }

    pub fn root(&self) -> &str {
        self.root.as_str()
    }

    pub fn dec_state(&self) -> &str {
        self.dec_state.as_str()
    }

    pub fn link(&self) -> &str {
        self.link.as_str()
    }

    pub fn group_blob(&self) -> &str {
        self.group_blob.as_str()
    }

    pub fn last_vote_round(&self) -> &str {
        self.last_vote_round.as_str()
    }

    pub fn last_qc(&self) -> &str {
        self.last_qc.as_str()
    }

    pub fn range(&self) -> &str {
        self.range.as_str()
    }

    pub fn commit_height(&self, height: u64) -> String {
        Self::join(&[self.link.as_str(), height.to_string().as_str()])
    }

    pub fn prepares(&self) -> &str {
        self.prepares.as_str()
    }

    pub fn pre_commits(&self) -> &str {
        self.pre_commits.as_str()
    }

    pub fn finish_proposals(&self) -> &str {
        self.finish_proposals.as_str()
    }

    pub fn flip_time(&self) -> &str {
        self.flip_time.as_str()
    }

    pub fn recycle(&self) -> &str {
        self.recycle.as_str()
    }

    pub fn adding(&self) -> &str {
        self.adding.as_str()
    }
}
