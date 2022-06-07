use cyfs_base::*;

pub enum PeerOfUnion {
    Left,
    Right
}

#[derive(RawEncode, RawDecode)]
pub struct UnionBalance {
    pub total: i64,
    pub left: i64,
    pub right: i64,
    pub deviation: i64
}

impl std::fmt::Display for UnionBalance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {}, {})", self.left, self.right, self.deviation, self.total)
    }
}

impl UnionBalance {
    pub fn default() -> UnionBalance {
        UnionBalance {
            total: 0,
            left: 0,
            right: 0,
            deviation: 0
        }
    }
}


pub enum FFSObjectState {
    Normal,
    Expire,
}