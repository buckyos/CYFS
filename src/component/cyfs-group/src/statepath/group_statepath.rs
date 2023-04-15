use cyfs_base::ObjectId;

pub const STATE_PATH_SEPARATOR: &str = "/";
pub const GROUP_STATE_PATH_DEC_STATE: &str = ".dec-state";
pub const GROUP_STATE_PATH_LINK: &str = ".link";
pub const GROUP_STATE_PATH_LAST_VOTE_ROUNDS: &str = "last-vote-round";
pub const GROUP_STATE_PATH_LAST_QC: &str = "last-qc";
pub const GROUP_STATE_PATH_LAST_TC: &str = "last-tc";
pub const GROUP_STATE_PATH_RANGE: &str = "range";
pub const GROUP_STATE_PATH_PREPARES: &str = "prepares";
pub const GROUP_STATE_PATH_PRE_COMMITS: &str = "pre-commits";
pub const GROUP_STATE_PATH_BLOCK: &str = "block";
pub const GROUP_STATE_PATH_RESULT_STATE: &str = "result-state";
pub const GROUP_STATE_PATH_FINISH_PROPOSALS: &str = "finish-proposals";
pub const GROUP_STATE_PATH_FLIP_TIME: &str = "flip-time";
pub const GROUP_STATE_PATH_RECYCLE: &str = "recycle";
pub const GROUP_STATE_PATH_ADDING: &str = "adding";

pub const STATEPATH_GROUP_DEC_RPATH: &str = ".update";
pub const STATEPATH_GROUP_DEC_LATEST_VERSION: &str = "latest-version";

pub struct GroupStatePath {
    rpath: String,
    root: String,
    dec_state: String,
    link: String,
    last_vote_round: String,
    last_qc: String,
    last_tc: String,
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
            last_tc: Self::join(&[
                "",
                rpath.as_str(),
                GROUP_STATE_PATH_LINK,
                GROUP_STATE_PATH_LAST_TC,
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

    pub fn last_vote_round(&self) -> &str {
        self.last_vote_round.as_str()
    }

    pub fn last_qc(&self) -> &str {
        self.last_qc.as_str()
    }

    pub fn last_tc(&self) -> &str {
        self.last_tc.as_str()
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

    pub fn prepares_block(&self, block_id: &ObjectId) -> String {
        Self::join(&[
            self.prepares.as_str(),
            block_id.to_string().as_str(),
            GROUP_STATE_PATH_BLOCK,
        ])
    }

    pub fn prepares_result_state(&self, block_id: &ObjectId) -> String {
        Self::join(&[
            self.prepares.as_str(),
            block_id.to_string().as_str(),
            GROUP_STATE_PATH_RESULT_STATE,
        ])
    }

    pub fn pre_commits(&self) -> &str {
        self.pre_commits.as_str()
    }

    pub fn pre_commits_block(&self, block_id: &ObjectId) -> String {
        Self::join(&[
            self.prepares.as_str(),
            block_id.to_string().as_str(),
            GROUP_STATE_PATH_BLOCK,
        ])
    }

    pub fn pre_commits_result_state(&self, block_id: &ObjectId) -> String {
        Self::join(&[
            self.prepares.as_str(),
            block_id.to_string().as_str(),
            GROUP_STATE_PATH_RESULT_STATE,
        ])
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
