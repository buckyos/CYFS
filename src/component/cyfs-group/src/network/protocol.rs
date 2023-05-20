use cyfs_base::*;
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath, HotstuffBlockQC};
use cyfs_group_lib::{GroupRPathStatus, HotstuffBlockQCVote, HotstuffTimeoutVote};
use cyfs_lib::NONObjectInfo;
use itertools::Itertools;

#[derive(RawEncode, RawDecode, PartialEq, Eq, Ord, Clone, Debug)]
pub enum SyncBound {
    Height(u64),
    Round(u64),
}

impl Copy for SyncBound {}

impl SyncBound {
    pub fn value(&self) -> u64 {
        match self {
            Self::Height(h) => *h,
            Self::Round(r) => *r,
        }
    }

    pub fn height(&self) -> u64 {
        match self {
            Self::Height(h) => *h,
            Self::Round(_r) => panic!("should be height"),
        }
    }

    pub fn round(&self) -> u64 {
        match self {
            Self::Round(r) => *r,
            Self::Height(_h) => panic!("should be round"),
        }
    }

    pub fn add(&self, value: u64) -> Self {
        match self {
            Self::Height(h) => Self::Height(*h + value),
            Self::Round(r) => Self::Round(*r + value),
        }
    }

    pub fn sub(&self, value: u64) -> Self {
        match self {
            Self::Height(h) => Self::Height(*h - value),
            Self::Round(r) => Self::Round(*r - value),
        }
    }
}

impl PartialOrd for SyncBound {
    fn partial_cmp(&self, other: &SyncBound) -> Option<std::cmp::Ordering> {
        let ord = match self {
            Self::Height(height) => match other {
                Self::Height(other_height) => height.cmp(other_height),
                Self::Round(other_round) => {
                    if height >= other_round {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Less
                    }
                }
            },
            Self::Round(round) => match other {
                Self::Round(other_round) => round.cmp(other_round),
                Self::Height(other_height) => {
                    if other_height >= round {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                }
            },
        };

        Some(ord)
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum HotstuffMessage {
    Block(cyfs_core::GroupConsensusBlock),
    BlockVote(HotstuffBlockQCVote),
    TimeoutVote(HotstuffTimeoutVote),
    Timeout(cyfs_core::HotstuffTimeout),

    SyncRequest(SyncBound, SyncBound), // [min, max]

    LastStateRequest,
    StateChangeNotify(GroupConsensusBlock, HotstuffBlockQC), // (block, qc)
    ProposalResult(
        ObjectId,
        BuckyResult<(Option<NONObjectInfo>, GroupConsensusBlock, HotstuffBlockQC)>,
    ), // (proposal-id, (ExecuteResult, block, qc))
    QueryState(String),
    VerifiableState(String, BuckyResult<GroupRPathStatus>),
}

impl std::fmt::Debug for HotstuffMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Block(block) => {
                write!(
                    f,
                    "HotstuffMessage::Block({}/{})",
                    block.block_id(),
                    block.round()
                )
            }
            Self::BlockVote(vote) => {
                write!(
                    f,
                    "HotstuffMessage::BlockVote({}/{})",
                    vote.block_id, vote.round
                )
            }
            Self::TimeoutVote(vote) => {
                write!(
                    f,
                    "HotstuffMessage::TimeoutVote({}/{})",
                    vote.round, vote.voter
                )
            }
            Self::Timeout(tc) => {
                write!(
                    f,
                    "HotstuffMessage::Timeout({}/{:?})",
                    tc.round,
                    tc.votes.iter().map(|v| v.voter).collect::<Vec<_>>()
                )
            }
            Self::SyncRequest(min, max) => {
                write!(f, "HotstuffMessage::SyncRequest([{:?}-{:?}])", min, max)
            }
            Self::StateChangeNotify(block, qc) => {
                write!(
                    f,
                    "HotstuffMessage::StateChangeNotify({}/{}, {}/{})",
                    block.block_id(),
                    block.round(),
                    qc.block_id,
                    qc.round
                )
            }
            Self::LastStateRequest => {
                write!(f, "HotstuffMessage::LastStateRequest",)
            }
            Self::ProposalResult(proposal_id, result) => {
                write!(
                    f,
                    "HotstuffMessage::ProposalResult({}, {:?})",
                    proposal_id,
                    result.as_ref().map(|(obj, block, qc)| {
                        format!(
                            "({:?}, {}/{}, {}/{})",
                            obj.as_ref().map(|o| o.object_id),
                            block.block_id(),
                            block.round(),
                            qc.block_id,
                            qc.round
                        )
                    })
                )
            }
            Self::QueryState(sub_path) => {
                write!(f, "HotstuffMessage::QueryState({})", sub_path)
            }
            Self::VerifiableState(sub_path, result) => {
                write!(
                    f,
                    "HotstuffMessage::VerifiableState({}, {:?})",
                    sub_path,
                    result.as_ref().map(|status| {
                        let desc = status.block_desc.content();
                        format!(
                            "({:?}/{:?}, {}/{}/{}) sub-count: {:?}",
                            desc.result_state_id(),
                            status.block_desc.object_id(),
                            desc.height(),
                            desc.round(),
                            status.certificate.round,
                            status.status_map.iter().map(|(key, _)| key).collect_vec()
                        )
                    })
                )
            }
        }
    }
}

const PACKAGE_FLAG_BITS: usize = 1;
const PACKAGE_FLAG_PROPOSAL_RESULT_OK: u8 = 0x80u8;
const PACKAGE_FLAG_QUERY_STATE_RESULT_OK: u8 = 0x80u8;

#[derive(Clone)]
pub(crate) enum HotstuffPackage {
    Block(cyfs_core::GroupConsensusBlock),
    BlockVote(ProtocolAddress, HotstuffBlockQCVote),
    TimeoutVote(ProtocolAddress, HotstuffTimeoutVote),
    Timeout(ProtocolAddress, cyfs_core::HotstuffTimeout),

    SyncRequest(ProtocolAddress, SyncBound, SyncBound),

    StateChangeNotify(GroupConsensusBlock, HotstuffBlockQC), // (block, qc)
    LastStateRequest(ProtocolAddress),
    ProposalResult(
        ObjectId,
        Result<
            (Option<NONObjectInfo>, GroupConsensusBlock, HotstuffBlockQC),
            (BuckyError, ProtocolAddress),
        >,
    ), // (proposal-id, ExecuteResult)
    QueryState(ProtocolAddress, String),
    VerifiableState(
        String,
        Result<GroupRPathStatus, (BuckyError, ProtocolAddress)>,
    ),
}

impl std::fmt::Debug for HotstuffPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Block(block) => {
                write!(
                    f,
                    "HotstuffPackage::Block({}/{})",
                    block.block_id(),
                    block.round()
                )
            }
            Self::BlockVote(_, vote) => {
                write!(
                    f,
                    "HotstuffPackage::BlockVote({}/{})",
                    vote.block_id, vote.round
                )
            }
            Self::TimeoutVote(_, vote) => {
                write!(
                    f,
                    "HotstuffPackage::TimeoutVote({}/{})",
                    vote.round, vote.voter
                )
            }
            Self::Timeout(_, tc) => {
                write!(
                    f,
                    "HotstuffPackage::Timeout({}/{:?})",
                    tc.round,
                    tc.votes.iter().map(|v| v.voter).collect::<Vec<_>>()
                )
            }
            Self::SyncRequest(_, min, max) => {
                write!(f, "HotstuffPackage::SyncRequest([{:?}-{:?}])", min, max)
            }
            Self::StateChangeNotify(block, qc) => {
                write!(
                    f,
                    "HotstuffPackage::StateChangeNotify({}/{}, {}/{})",
                    block.block_id(),
                    block.round(),
                    qc.block_id,
                    qc.round
                )
            }
            Self::LastStateRequest(_) => {
                write!(f, "HotstuffPackage::LastStateRequest",)
            }
            Self::ProposalResult(proposal_id, result) => {
                write!(
                    f,
                    "HotstuffPackage::ProposalResult({}, {:?})",
                    proposal_id,
                    result.as_ref().map_or_else(
                        |(err, _)| { Err(err) },
                        |(obj, block, qc)| {
                            let ok = format!(
                                "({:?}, {}/{}, {}/{})",
                                obj.as_ref().map(|o| o.object_id),
                                block.block_id(),
                                block.round(),
                                qc.block_id,
                                qc.round
                            );
                            Ok(ok)
                        }
                    )
                )
            }
            Self::QueryState(_, sub_path) => {
                write!(f, "HotstuffPackage::QueryState({})", sub_path)
            }
            Self::VerifiableState(sub_path, result) => {
                write!(
                    f,
                    "HotstuffPackage::VerifiableState({}, {:?})",
                    sub_path,
                    result.as_ref().map_or_else(
                        |(err, _)| { Err(err) },
                        |status| {
                            let desc = status.block_desc.content();
                            let ok = format!(
                                "({:?}/{:?}, {}/{}/{}) sub-count: {:?}",
                                desc.result_state_id(),
                                status.block_desc.object_id(),
                                desc.height(),
                                desc.round(),
                                status.certificate.round,
                                status.status_map.iter().map(|(key, _)| key).collect_vec()
                            );
                            Ok(ok)
                        }
                    )
                )
            }
        }
    }
}

impl HotstuffPackage {
    pub(crate) fn rpath(&self) -> &GroupRPath {
        match self {
            HotstuffPackage::Block(block) => block.rpath(),
            HotstuffPackage::BlockVote(addr, _) => addr.check_rpath(),
            HotstuffPackage::TimeoutVote(addr, _) => addr.check_rpath(),
            HotstuffPackage::Timeout(addr, _) => addr.check_rpath(),
            HotstuffPackage::SyncRequest(addr, _, _) => addr.check_rpath(),
            HotstuffPackage::StateChangeNotify(block, _) => block.rpath(),
            HotstuffPackage::LastStateRequest(addr) => addr.check_rpath(),
            HotstuffPackage::ProposalResult(_, result) => result.as_ref().map_or_else(
                |(_, addr)| addr.check_rpath(),
                |(_, block, _)| block.rpath(),
            ),
            HotstuffPackage::QueryState(addr, _) => addr.check_rpath(),
            HotstuffPackage::VerifiableState(_, result) => result.as_ref().map_or_else(
                |(_, addr)| addr.check_rpath(),
                |status| status.block_desc.content().rpath(),
            ),
        }
    }
}

fn encode_with_length<'a, O: RawEncode>(
    buf: &'a mut [u8],
    obj: &O,
    purpose: &Option<RawEncodePurpose>,
    length_size: usize,
) -> BuckyResult<&'a mut [u8]> {
    let (len_buf, buf) = buf.split_at_mut(length_size);
    let before_len = buf.len();
    let buf = obj.raw_encode(buf, purpose)?;
    let len = before_len - buf.len();
    assert!(len <= (1 << (length_size << 3)) - 1);
    len_buf.copy_from_slice(&len.to_le_bytes()[..length_size]);

    Ok(buf)
}

fn decode_with_length<'de, O: RawDecode<'de>>(
    buf: &'de [u8],
    length_size: usize,
) -> BuckyResult<(O, &'de [u8])> {
    assert!(length_size <= 4);
    let (len_buf, buf) = buf.split_at(length_size);

    let mut len_buf_4 = [0u8; 4];
    len_buf_4[..length_size].copy_from_slice(len_buf);
    let len = u32::from_le_bytes(len_buf_4) as usize;

    let _before_len = buf.len();
    let (obj, remain) = O::raw_decode(&buf[..len])?;
    assert_eq!(remain.len(), 0);

    Ok((obj, &buf[len..]))
}

impl RawEncode for HotstuffPackage {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let len = match self {
            HotstuffPackage::Block(b) => 3 + b.raw_measure(purpose)?,
            HotstuffPackage::BlockVote(addr, vote) => {
                2 + addr.raw_measure(purpose)? + 3 + vote.raw_measure(purpose)?
            }
            HotstuffPackage::TimeoutVote(addr, vote) => {
                2 + addr.raw_measure(purpose)? + 3 + vote.raw_measure(purpose)?
            }
            HotstuffPackage::Timeout(addr, tc) => {
                2 + addr.raw_measure(purpose)? + 3 + tc.raw_measure(purpose)?
            }
            HotstuffPackage::SyncRequest(addr, min, max) => {
                2 + addr.raw_measure(purpose)?
                    + min.raw_measure(purpose)?
                    + max.raw_measure(purpose)?
            }
            HotstuffPackage::StateChangeNotify(block, qc) => {
                3 + block.raw_measure(purpose)? + 3 + qc.raw_measure(purpose)?
            }
            HotstuffPackage::LastStateRequest(addr) => 2 + addr.raw_measure(purpose)?,
            HotstuffPackage::ProposalResult(id, result) => {
                id.raw_measure(purpose)?
                    + match result {
                        Ok((non, block, block_qc)) => {
                            non.raw_measure(purpose)?
                                + 3
                                + block.raw_measure(purpose)?
                                + 3
                                + block_qc.raw_measure(purpose)?
                        }
                        Err((err, addr)) => {
                            err.raw_measure(purpose)? + 2 + addr.raw_measure(purpose)?
                        }
                    }
            }
            HotstuffPackage::QueryState(addr, sub_path) => {
                2 + addr.raw_measure(purpose)? + sub_path.raw_measure(purpose)?
            }
            HotstuffPackage::VerifiableState(sub_path, result) => {
                sub_path.raw_measure(purpose)?
                    + match result {
                        Ok(status) => 3 + status.raw_measure(purpose)?,
                        Err((err, addr)) => {
                            err.raw_measure(purpose)? + 2 + addr.raw_measure(purpose)?
                        }
                    }
            }
        };

        Ok(1 + len)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            HotstuffPackage::Block(b) => {
                buf[0] = 0;
                let buf = &mut buf[1..];
                encode_with_length(buf, b, purpose, 3)
            }
            HotstuffPackage::BlockVote(addr, vote) => {
                buf[0] = 1;
                let buf = &mut buf[1..];
                let buf = encode_with_length(buf, addr, purpose, 2)?;
                encode_with_length(buf, vote, purpose, 3)
            }
            HotstuffPackage::TimeoutVote(addr, vote) => {
                buf[0] = 2;
                let buf = &mut buf[1..];
                let buf = encode_with_length(buf, addr, purpose, 2)?;
                encode_with_length(buf, vote, purpose, 3)
            }
            HotstuffPackage::Timeout(addr, tc) => {
                buf[0] = 3;
                let buf = &mut buf[1..];
                let buf = encode_with_length(buf, addr, purpose, 2)?;
                encode_with_length(buf, tc, purpose, 3)
            }
            HotstuffPackage::SyncRequest(addr, min, max) => {
                buf[0] = 4;
                let buf = &mut buf[1..];
                let buf = encode_with_length(buf, addr, purpose, 2)?;
                let buf = min.raw_encode(buf, purpose)?;
                max.raw_encode(buf, purpose)
            }
            HotstuffPackage::StateChangeNotify(block, qc) => {
                buf[0] = 5;
                let buf = &mut buf[1..];
                let buf = encode_with_length(buf, block, purpose, 3)?;
                encode_with_length(buf, qc, purpose, 3)
            }
            HotstuffPackage::LastStateRequest(addr) => {
                buf[0] = 6;
                let buf = &mut buf[1..];
                encode_with_length(buf, addr, purpose, 2)
            }
            HotstuffPackage::ProposalResult(id, result) => {
                buf[0] = 7;
                if result.is_ok() {
                    buf[0] |= PACKAGE_FLAG_PROPOSAL_RESULT_OK;
                }

                let buf = &mut buf[1..];
                let buf = id.raw_encode(buf, purpose)?;
                match result {
                    Ok((non, block, qc)) => {
                        let buf = non.raw_encode(buf, purpose)?;
                        let buf = encode_with_length(buf, block, purpose, 3)?;
                        encode_with_length(buf, qc, purpose, 3)
                    }
                    Err((err, addr)) => {
                        let buf = err.raw_encode(buf, purpose)?;
                        encode_with_length(buf, addr, purpose, 2)
                    }
                }
            }
            HotstuffPackage::QueryState(addr, sub_path) => {
                buf[0] = 8;
                let buf = &mut buf[1..];
                encode_with_length(buf, addr, purpose, 2)?;
                sub_path.raw_encode(buf, purpose)
            }
            HotstuffPackage::VerifiableState(sub_path, result) => {
                buf[0] = 9;
                if result.is_ok() {
                    buf[0] |= PACKAGE_FLAG_QUERY_STATE_RESULT_OK;
                }
                let buf = &mut buf[1..];
                let buf = sub_path.raw_encode(buf, purpose)?;
                match result {
                    Ok(status) => encode_with_length(buf, status, purpose, 3),
                    Err((err, addr)) => {
                        let buf = err.raw_encode(buf, purpose)?;
                        encode_with_length(buf, addr, purpose, 2)
                    }
                }
            }
        }
    }
}

impl<'de> RawDecode<'de> for HotstuffPackage {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let pkg_type = buf[0] << PACKAGE_FLAG_BITS >> PACKAGE_FLAG_BITS;
        // let pkg_flag = buf[0] - pkg_type;

        match pkg_type {
            0 => {
                let buf = &buf[1..];
                let (b, buf) = decode_with_length(buf, 3)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::Block(b), buf))
            }
            1 => {
                let buf = &buf[1..];
                let (addr, buf) = decode_with_length(buf, 2)?;
                let (vote, buf) = decode_with_length(buf, 3)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::BlockVote(addr, vote), buf))
            }
            2 => {
                let buf = &buf[1..];
                let (addr, buf) = decode_with_length(buf, 2)?;
                let (vote, buf) = decode_with_length(buf, 3)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::TimeoutVote(addr, vote), buf))
            }
            3 => {
                let buf = &buf[1..];
                let (addr, buf) = decode_with_length(buf, 2)?;
                let (vote, buf) = decode_with_length(buf, 3)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::Timeout(addr, vote), buf))
            }
            4 => {
                let buf = &buf[1..];
                let (addr, buf) = decode_with_length(buf, 2)?;
                let (min, buf) = SyncBound::raw_decode(buf)?;
                let (max, buf) = SyncBound::raw_decode(buf)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::SyncRequest(addr, min, max), buf))
            }
            5 => {
                let buf = &buf[1..];
                let (block, buf) = decode_with_length(buf, 3)?;
                let (qc, buf) = decode_with_length(buf, 3)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::StateChangeNotify(block, qc), buf))
            }
            6 => {
                let buf = &buf[1..];
                let (addr, buf) = decode_with_length(buf, 2)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::LastStateRequest(addr), buf))
            }
            7 => {
                let is_ok = (buf[0] & PACKAGE_FLAG_PROPOSAL_RESULT_OK) != 0;
                let buf = &buf[1..];
                let (id, buf) = ObjectId::raw_decode(buf)?;
                match is_ok {
                    true => {
                        let (non, buf) = Option::<NONObjectInfo>::raw_decode(buf)?;
                        let (block, buf) = decode_with_length(buf, 3)?;
                        let (qc, buf) = decode_with_length(buf, 3)?;
                        assert_eq!(buf.len(), 0);
                        Ok((
                            HotstuffPackage::ProposalResult(id, Ok((non, block, qc))),
                            buf,
                        ))
                    }
                    false => {
                        let (err, buf) = BuckyError::raw_decode(buf)?;
                        let (addr, buf) = decode_with_length(buf, 2)?;
                        assert_eq!(buf.len(), 0);
                        Ok((HotstuffPackage::ProposalResult(id, Err((err, addr))), buf))
                    }
                }
            }
            8 => {
                let buf = &buf[1..];
                let (addr, buf) = decode_with_length(buf, 2)?;
                let (sub_path, buf) = String::raw_decode(buf)?;
                assert_eq!(buf.len(), 0);
                Ok((HotstuffPackage::QueryState(addr, sub_path), buf))
            }
            9 => {
                let is_ok = (buf[0] & PACKAGE_FLAG_QUERY_STATE_RESULT_OK) != 0;
                let buf = &buf[1..];
                let (sub_path, buf) = String::raw_decode(buf)?;
                match is_ok {
                    true => {
                        let (status, buf) = decode_with_length(buf, 3)?;
                        assert_eq!(buf.len(), 0);
                        Ok((HotstuffPackage::VerifiableState(sub_path, Ok(status)), buf))
                    }
                    false => {
                        let (err, buf) = BuckyError::raw_decode(buf)?;
                        let (addr, buf) = decode_with_length(buf, 2)?;
                        assert_eq!(buf.len(), 0);
                        Ok((
                            HotstuffPackage::VerifiableState(sub_path, Err((err, addr))),
                            buf,
                        ))
                    }
                }
            }
            _ => unreachable!("unknown protocol"),
        }
    }
}

impl HotstuffPackage {
    pub fn from_msg(msg: HotstuffMessage, rpath: GroupRPath) -> Self {
        match msg {
            HotstuffMessage::Block(block) => HotstuffPackage::Block(block),
            HotstuffMessage::BlockVote(vote) => {
                HotstuffPackage::BlockVote(ProtocolAddress::Full(rpath), vote)
            }
            HotstuffMessage::TimeoutVote(vote) => {
                HotstuffPackage::TimeoutVote(ProtocolAddress::Full(rpath), vote)
            }
            HotstuffMessage::Timeout(tc) => {
                HotstuffPackage::Timeout(ProtocolAddress::Full(rpath), tc)
            }
            HotstuffMessage::SyncRequest(min_bound, max_bound) => {
                HotstuffPackage::SyncRequest(ProtocolAddress::Full(rpath), min_bound, max_bound)
            }
            HotstuffMessage::LastStateRequest => {
                HotstuffPackage::LastStateRequest(ProtocolAddress::Full(rpath))
            }
            HotstuffMessage::StateChangeNotify(header_block, qc_block) => {
                HotstuffPackage::StateChangeNotify(header_block, qc_block)
            }
            HotstuffMessage::ProposalResult(proposal_id, result) => {
                HotstuffPackage::ProposalResult(
                    proposal_id,
                    result.map_err(|err| (err, ProtocolAddress::Full(rpath))),
                )
            }
            HotstuffMessage::QueryState(sub_path) => {
                HotstuffPackage::QueryState(ProtocolAddress::Full(rpath), sub_path)
            }
            HotstuffMessage::VerifiableState(sub_path, result) => HotstuffPackage::VerifiableState(
                sub_path,
                result.map_err(|err| (err, ProtocolAddress::Full(rpath))),
            ),
        }
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) enum ProtocolAddress {
    Full(GroupRPath),
    Channel(u64),
}

impl ProtocolAddress {
    pub fn check_rpath(&self) -> &GroupRPath {
        match self {
            ProtocolAddress::Full(rpath) => rpath,
            ProtocolAddress::Channel(_) => panic!("no rpath"),
        }
    }
}

#[cfg(test)]
mod test {}
