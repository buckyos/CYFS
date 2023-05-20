use std::collections::HashSet;

use cyfs_base::{
    verify_object_desc_sign, BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject,
    ObjectDesc, ObjectId, OwnerObjectDesc, RsaCPUObjectVerifier, SignatureSource,
    SingleKeyObjectDesc, Verifier,
};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockDesc, GroupConsensusBlockDescContent,
    GroupConsensusBlockObject, HotstuffBlockQC, HotstuffTimeout,
};
use cyfs_group_lib::{HotstuffBlockQCVote, HotstuffTimeoutVote};

use crate::{network::NONDriverHelper, storage::GroupShellManager};

#[derive(Clone)]
pub(crate) struct Committee {
    group_id: ObjectId,
    non_driver: NONDriverHelper,
    shell_mgr: GroupShellManager,
    local_device_id: ObjectId,
}

impl Committee {
    pub fn new(
        group_id: ObjectId,
        non_driver: NONDriverHelper,
        shell_mgr: GroupShellManager,
        local_device_id: ObjectId,
    ) -> Self {
        Committee {
            group_id,
            non_driver,
            shell_mgr,
            local_device_id,
        }
    }

    pub async fn quorum_threshold(
        &self,
        voters: &HashSet<ObjectId>,
        group_shell_id: Option<&ObjectId>,
    ) -> BuckyResult<bool> {
        let group = self
            .shell_mgr
            .get_group(&self.group_id, group_shell_id, None)
            .await?;
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
        group_shell_id: Option<&ObjectId>,
        round: u64,
        remote: Option<&ObjectId>,
    ) -> BuckyResult<ObjectId> {
        let group = if group_shell_id.is_none() {
            self.shell_mgr.group().0
        } else {
            self.shell_mgr
                .get_group(&self.group_id, group_shell_id, remote)
                .await?
        };
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
        let block_id = block.block_id();
        if !block.check() {
            log::warn!(
                "[group committee] error block with invalid content: {}",
                block_id
            )
        }

        log::debug!(
            "[group committee] {} verify block {} step1",
            self.local_device_id,
            block_id
        );

        let group = self
            .shell_mgr
            .get_group(&self.group_id, Some(block.group_shell_id()), Some(&from))
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

        log::debug!(
            "[group committee] {} verify block {} step2",
            self.local_device_id,
            block_id
        );

        let _prev_block = if let Some(qc) = block.qc() {
            let prev_block = self
                .non_driver
                .get_block(&qc.block_id, Some(&from))
                .await
                .map_err(|err| {
                    log::error!(
                        "get the prev-block({}) for verify block({}) failed: {:?}",
                        qc.block_id,
                        block_id,
                        err
                    );
                    err
                })?;
            self.verify_qc(qc, &prev_block).await?;
            Some(prev_block)
        } else {
            None
        };

        log::debug!(
            "[group committee] {} verify block {} step3",
            self.local_device_id,
            block_id
        );

        if let Some(tc) = block.tc() {
            self.verify_tc(tc, block.group_shell_id()).await?;
        }

        log::debug!(
            "[group committee] {} verify block {} step4",
            self.local_device_id,
            block_id
        );

        Ok(())
    }

    pub async fn verify_block_desc_with_qc(
        &self,
        block_desc: &GroupConsensusBlockDesc,
        qc: &HotstuffBlockQC,
        from: ObjectId,
    ) -> BuckyResult<()> {
        let block_id = block_desc.object_id();

        log::debug!(
            "[group committee] {} verify block desc {} step1",
            self.local_device_id,
            block_id
        );

        if block_id != qc.block_id {
            return Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                "the block id is unmatch with the qc",
            ));
        }

        self.shell_mgr
            .get_group(
                &self.group_id,
                Some(block_desc.content().group_shell_id()),
                Some(&from),
            )
            .await?;

        log::debug!(
            "[group committee] {} verify block desc {} step2",
            self.local_device_id,
            block_id
        );

        self.verify_qc_with_desc(qc, block_desc.content()).await?;

        log::debug!(
            "[group committee] {} verify block desc {} step3",
            self.local_device_id,
            block_id
        );

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
        group_shell_id: &ObjectId,
    ) -> BuckyResult<()> {
        let tc_group_shell_id = tc.group_shell_id.as_ref().unwrap_or(group_shell_id);

        let is_enough = self
            .quorum_threshold(
                &tc.votes.iter().map(|v| v.voter).collect(),
                Some(tc_group_shell_id),
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
            let hash =
                HotstuffTimeoutVote::hash_content(vote.high_qc_round, tc.round, tc_group_shell_id);
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
        self.verify_qc_with_desc(qc, prev_block.named_object().desc().content())
            .await
    }

    pub async fn verify_qc_with_desc(
        &self,
        qc: &HotstuffBlockQC,
        prev_block_desc: &GroupConsensusBlockDescContent,
    ) -> BuckyResult<()> {
        if qc.round != prev_block_desc.round() {
            log::warn!("[group committee] round is not match with prev-block in qc, round: {}, prev_round: {}", qc.round, prev_block_desc.round());
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                "round not match in qc",
            ));
        }

        let is_enough = self
            .quorum_threshold(
                &qc.votes.iter().map(|v| v.voter).collect(),
                Some(prev_block_desc.group_shell_id()),
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
