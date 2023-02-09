use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use async_std::sync::RwLock;
use cyfs_base::{
    verify_object_desc_sign, BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject,
    ObjectDesc, ObjectId, OwnerObjectDesc, RsaCPUObjectVerifier, SignatureSource,
    SingleKeyObjectDesc, Verifier,
};
use cyfs_chunk_lib::ChunkMeta;
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, HotstuffBlockQC, HotstuffTimeout};

use crate::{network::NONDriverHelper, HotstuffBlockQCVote, HotstuffTimeoutVote};

#[derive(Clone)]
pub(crate) struct Committee {
    group_id: ObjectId,
    non_driver: NONDriverHelper,
    group_cache: Arc<RwLock<HashMap<ObjectId, Group>>>, // (group_chunk_id, group)
}

impl Committee {
    pub fn new(group_id: ObjectId, non_driver: NONDriverHelper) -> Self {
        Committee {
            group_id,
            non_driver,
            group_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_group(&self, group_chunk_id: Option<&ObjectId>) -> BuckyResult<Group> {
        self.check_group(group_chunk_id, None).await
    }

    pub async fn quorum_threshold(
        &self,
        voters: &HashSet<ObjectId>,
        group_chunk_id: Option<&ObjectId>,
    ) -> BuckyResult<bool> {
        let group = self.check_group(group_chunk_id, None).await?;
        let voters: Vec<&ObjectId> = voters
            .iter()
            .filter(|id| {
                group
                    .ood_list()
                    .iter()
                    .find(|mem| mem.object_id() == *id)
                    .is_some()
            })
            .collect();

        let is_enough = voters.len() >= ((group.ood_list().len() << 1) / 3 + 1);
        Ok(is_enough)
    }

    pub async fn get_leader(
        &self,
        group_chunk_id: Option<&ObjectId>,
        round: u64,
    ) -> BuckyResult<ObjectId> {
        let group = self.check_group(group_chunk_id, None).await?;
        let i = (round % (group.ood_list().len() as u64)) as usize;
        Ok(group.ood_list()[i].object_id().clone())
    }

    pub async fn verify_block(
        &self,
        block: &GroupConsensusBlock,
        from: ObjectId,
    ) -> BuckyResult<()> {
        /* *
         * 验证block下的签名是否符合对上一个block归属group的确认
         */
        if !block.check() {
            log::warn!(
                "[group committee] error block with invalid content: {}",
                block.named_object().desc().calculate_id()
            )
        }

        let group = self
            .check_group(Some(block.group_chunk_id()), Some(&from))
            .await?;

        if !self.check_block_sign(&block, &group).await? {
            log::warn!(
                "[group committee] check signature failed: {}",
                block.named_object().desc().calculate_id()
            );
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "invalid signature",
            ));
        }

        let prev_block = if let Some(qc) = block.qc() {
            let prev_block = self.non_driver.get_block(&qc.block_id, None).await?;
            self.verify_qc(qc, &prev_block).await?;
            Some(prev_block)
        } else {
            None
        };

        if let Some(tc) = block.tc() {
            self.verify_tc(tc, prev_block.as_ref()).await?;
        }

        Ok(())
    }

    pub async fn verify_vote(&self, vote: &HotstuffBlockQCVote) -> BuckyResult<()> {
        let hash = vote.hash();
        let device = self.non_driver.get_device(&vote.voter).await?;
        let verifier = RsaCPUObjectVerifier::new(device.desc().public_key().clone());
        let is_ok = verifier.verify(hash.as_slice(), &vote.signature).await;
        if !is_ok {
            log::warn!("[group committee] vote with error signature");
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "invalid signature",
            ));
        }
        Ok(())
    }

    pub async fn verify_timeout(
        &self,
        vote: &HotstuffTimeoutVote,
        prev_block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        // 用block验vote.high_qc
        let hash = vote.hash();
        let device = self.non_driver.get_device(&vote.voter).await?;
        let verifier = RsaCPUObjectVerifier::new(device.desc().public_key().clone());
        let is_ok = verifier.verify(hash.as_slice(), &vote.signature).await;
        if !is_ok {
            log::warn!("[group committee] vote with error signature");
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "invalid signature",
            ));
        }

        if let Some(high_qc) = vote.high_qc.as_ref() {
            let prev_block = prev_block.expect("no block for high-qc in timeout");
            if prev_block.round() != high_qc.round {
                log::warn!("[group committee] vote with error round in timeout");
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidSignature,
                    "invalid round",
                ));
            }
            self.verify_qc(high_qc, prev_block).await?;
        }
        Ok(())
    }

    pub async fn verify_tc(
        &self,
        tc: &HotstuffTimeout,
        prev_block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        let highest_round = tc
            .votes
            .iter()
            .map(|v| v.high_qc_round)
            .max()
            .map_or(0, |round| round);
        let prev_round = prev_block.map_or(0, |b| b.round());
        if highest_round != prev_round {
            log::warn!("[group committee] hightest round is not match with prev-block in tc, highest_round: {:?}, prev_round: {:?}", highest_round, prev_round);
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                "round not match in tc",
            ));
        }

        let is_enough = self
            .quorum_threshold(
                &tc.votes.iter().map(|v| v.voter).collect(),
                prev_block.map(|b| b.group_chunk_id()),
            )
            .await?;

        if !is_enough {
            log::warn!("[group committee] tc with vote not enough.");
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "not enough",
            ));
        }

        let verify_vote_results = futures::future::join_all(tc.votes.iter().map(|vote| async {
            let hash = HotstuffTimeoutVote::hash_content(vote.high_qc_round, tc.round);
            match self.non_driver.get_device(&vote.voter).await {
                Ok(device) => {
                    let verifier = RsaCPUObjectVerifier::new(device.desc().public_key().clone());
                    let is_ok = verifier.verify(hash.as_slice(), &vote.signature).await;
                    if !is_ok {
                        log::warn!("[group committee] vote with error signature");
                        Err(BuckyError::new(
                            BuckyErrorCode::InvalidSignature,
                            "invalid signature",
                        ))
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(e),
            }
        }))
        .await;

        verify_vote_results
            .into_iter()
            .find(|r| r.is_err())
            .map_or(Ok(()), |e| e)
    }

    pub async fn verify_qc(
        &self,
        qc: &HotstuffBlockQC,
        prev_block: &GroupConsensusBlock,
    ) -> BuckyResult<()> {
        if qc.round != prev_block.round() {
            log::warn!("[group committee] round is not match with prev-block in qc, round: {}, prev_round: {}", qc.round, prev_block.round());
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                "round not match in qc",
            ));
        }

        let is_enough = self
            .quorum_threshold(
                &qc.votes.iter().map(|v| v.voter).collect(),
                Some(prev_block.group_chunk_id()),
            )
            .await?;

        if !is_enough {
            log::warn!("[group committee] qc with vote not enough.");
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "not enough",
            ));
        }

        let vote_verify_results = futures::future::join_all(qc.votes.iter().map(|vote| async {
            let vote = HotstuffBlockQCVote {
                block_id: qc.block_id,
                prev_block_id: qc.prev_block_id.clone(),
                round: qc.round,
                voter: vote.voter,
                signature: vote.signature.clone(),
            };
            self.verify_vote(&vote).await
        }))
        .await;

        vote_verify_results
            .into_iter()
            .find(|r| r.is_err())
            .map_or(Ok(()), |e| e)
    }

    async fn check_group(
        &self,
        chunk_id: Option<&ObjectId>,
        from: Option<&ObjectId>,
    ) -> BuckyResult<Group> {
        {
            // read
            let cache = self.group_cache.read().await;
            if let Some(chunk_id) = chunk_id {
                if let Some(group) = cache.get(chunk_id) {
                    return Ok(group.clone());
                }
            }
        }

        let group = self
            .non_driver
            .get_group(&self.group_id, chunk_id, from)
            .await?;

        let group_chunk = ChunkMeta::from(&group).to_chunk().await?;
        let calc_id = group_chunk.calculate_id().object_id();
        if let Some(id) = chunk_id {
            assert_eq!(&calc_id, id);
        }

        {
            // write
            let mut cache = self.group_cache.write().await;
            match cache.entry(calc_id) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if entry.get().version() < group.version() {
                        entry.insert(group.clone());
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(group.clone());
                }
            }
        }

        Ok(group)
    }

    async fn check_block_sign(
        &self,
        block: &GroupConsensusBlock,
        group: &Group,
    ) -> BuckyResult<bool> {
        let signs = match block.named_object().signs().desc_signs() {
            Some(signs) if signs.len() > 0 => signs,
            _ => {
                log::warn!("[group committee] no signatures");
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidSignature,
                    "not signature",
                ));
            }
        };

        let mut sign_device = None;

        for sign in signs {
            if let SignatureSource::Object(obj) = sign.sign_source() {
                if let Ok(device) = self.non_driver.get_device(&obj.obj_id).await {
                    let device_id = device.desc().device_id();
                    if group.ood_list().contains(&device_id)
                        && device.desc().owner().and_then(|dev_owner| {
                            block
                                .named_object()
                                .desc()
                                .owner()
                                .and_then(|blk_owner| Some(blk_owner == dev_owner))
                        }) == Some(true)
                    {
                        sign_device = Some((device, sign));
                        break;
                    } else {
                        log::warn!("[group committee] block signed by invalid object.");
                    }
                }
            } else {
                log::warn!("[group committee] support the SignatureSource::Object only.");
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidSignature,
                    "not SignatureSource::Object",
                ));
            }
        }

        let sign_device = match sign_device {
            Some(device) => device,
            None => {
                log::warn!("[group committee] not found the sign device.");
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidSignature,
                    "not found device",
                ));
            }
        };

        let verifier = RsaCPUObjectVerifier::new(sign_device.0.desc().public_key().clone());
        verify_object_desc_sign(&verifier, block.named_object(), &sign_device.1).await
    }
}
