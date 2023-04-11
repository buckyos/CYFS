use std::collections::HashSet;

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject, ObjectDesc, ObjectId,
    ObjectTypeCode, People, PeopleId, RawConvertTo, RsaCPUObjectVerifier, Signature,
    SingleKeyObjectDesc, Verifier,
};
use cyfs_core::GroupBlob;

async fn verify_signature(
    signs: Option<&Vec<Signature>>,
    data_buf: &[u8],
    verifier: &RsaCPUObjectVerifier,
    signer_id: &PeopleId,
) -> BuckyResult<()> {
    let signs = signs.map_or([].as_slice(), |s| s.as_slice());
    let sign = signs.iter().find(|s| match s.sign_source() {
        cyfs_base::SignatureSource::Object(signer) => &signer.obj_id == signer_id.object_id(),
        _ => false,
    });

    match sign {
        Some(sign) => {
            if verifier.verify(data_buf, sign).await {
                Ok(())
            } else {
                let msg = format!("Invalid signature from {}", signer_id);
                log::warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg))
            }
        }
        None => {
            let msg = format!("Not found signature from {}", signer_id);
            log::warn!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }
}

async fn verify_group_signature(
    group: &Group,
    people_id: &PeopleId,
    member_querier: &impl MemberQuerier,
) -> BuckyResult<()> {
    let people = member_querier.get_people(people_id).await?;
    let verifier = RsaCPUObjectVerifier::new(people.desc().public_key().clone());
    let desc_buf = group.desc().to_vec()?;
    verify_signature(
        group.signs().desc_signs(),
        desc_buf.as_slice(),
        &verifier,
        people_id,
    )
    .await?;
    let body_buf = group.body().to_vec()?;
    verify_signature(
        group.signs().body_signs(),
        body_buf.as_slice(),
        &verifier,
        people_id,
    )
    .await
}

#[async_trait::async_trait]
pub trait MemberQuerier: Send + Sync {
    async fn get_people(&self, people_id: &PeopleId) -> BuckyResult<People>;
}

#[async_trait::async_trait]
pub trait GroupVerifier {
    // Check the update is allowed
    async fn is_update_valid(
        &self,
        latest_group: Option<&Group>,
        member_querier: &impl MemberQuerier,
    ) -> BuckyResult<()>;
}

#[async_trait::async_trait]
impl GroupVerifier for Group {
    async fn is_update_valid(
        &self,
        latest_group: Option<&Group>,
        member_querier: &impl MemberQuerier,
    ) -> BuckyResult<()> {
        let group_id = self.desc().object_id();

        if self.admins().len() == 0 {
            let msg = format!("Update group({}) with no admins.", group_id);
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        let (last_admins, last_members) = match latest_group {
            Some(latest_group) => {
                let latest_group_id = latest_group.desc().object_id();
                if group_id != latest_group_id {
                    let msg = format!(
                        "The new group({}) is unmatch with the latest group({}).",
                        group_id, latest_group_id
                    );
                    log::warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }

                let latest_group_blob = latest_group.to_blob();
                let latest_group_blob_id = latest_group_blob.desc().object_id();
                if self.version() != latest_group.version() + 1
                    || self.prev_blob_id() != &Some(latest_group_blob_id)
                {
                    let msg = format!("Attempt to update group({}) from unknown version({}/{:?}), latest version: {}/{}.", group_id, self.version() - 1, self.prev_blob_id(), latest_group.version(), latest_group_blob_id);
                    log::warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }

                (
                    HashSet::<ObjectId>::from_iter(
                        latest_group
                            .admins()
                            .keys()
                            .filter(|m| m.obj_type_code() == ObjectTypeCode::People)
                            .map(|m| *m),
                    ),
                    HashSet::<ObjectId>::from_iter(
                        latest_group
                            .members()
                            .keys()
                            .filter(|m| m.obj_type_code() == ObjectTypeCode::People)
                            .map(|m| *m),
                    ),
                )
            }
            None => match self.prev_blob_id() {
                Some(prev_blob_id) => {
                    let msg = format!(
                        "The latest group({}) is necessary for update. prev_blob_id: {}",
                        group_id, prev_blob_id
                    );
                    log::warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
                None => {
                    if let Some(founder) = self.founder_id() {
                        if self.admins().values().find(|m| &m.id == founder).is_none() {
                            let msg = format!(
                                "Update group({}) the founder({}) must be an administrator.",
                                group_id, founder
                            );
                            log::warn!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
                        }
                    }
                    (HashSet::new(), HashSet::new())
                }
            },
        };

        // admins: > 1/2
        // new members: all

        let add_admins = HashSet::<ObjectId>::from_iter(
            self.admins()
                .keys()
                .filter(|m| {
                    m.obj_type_code() == ObjectTypeCode::People && !last_admins.contains(*m)
                })
                .map(|m| *m),
        );
        let add_members = HashSet::<ObjectId>::from_iter(
            self.members()
                .keys()
                .filter(|m| {
                    m.obj_type_code() == ObjectTypeCode::People && !last_members.contains(*m)
                })
                .map(|m| *m),
        );

        if add_admins.len() != self.admins().len() - last_admins.len() {
            let msg = format!(
                "Update group({}) with duplicate admins or invalid admins.",
                group_id
            );
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        if add_members.len() != self.members().len() - add_members.len() {
            let msg = format!(
                "Update group({}) with duplicate members or invalid members.",
                group_id
            );
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        let additionals = add_admins
            .union(&add_members)
            .map(|id| *id)
            .collect::<Vec<_>>();
        if additionals.len() != add_admins.len() + add_members.len() {
            let msg = format!(
                "Update group({}) with admins is not necessary in members.",
                group_id
            );
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        let check_peoples =
            futures::future::join_all(last_admins.iter().chain(additionals.iter()).map(
                |people_id| async move {
                    let people_id = PeopleId::try_from(people_id).unwrap();
                    verify_group_signature(self, &people_id, member_querier).await
                },
            ))
            .await;

        let (last_admin_signs, add_member_signs) = check_peoples.split_at(last_admins.len());
        let last_admin_sign_count = last_admin_signs.iter().filter(|s| s.is_ok()).count();
        if last_admin_sign_count <= last_admins.len() / 2 {
            let msg = format!(
                "Update group({}) failed for signatures from admins in latest version is not enough: expected {}, got {}.",
                group_id,
                last_admins.len() / 2 + 1,
                last_admin_sign_count
            );
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg));
        }

        let failed_add_member_sign_pos = add_member_signs.iter().position(|s| s.is_err());
        match failed_add_member_sign_pos {
            Some(pos) => {
                let msg = format!(
                    "Update group({}) failed for signatures from additional member({:?}) is invalid.",
                    group_id,
                    additionals.get(pos)
                );
                log::warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg));
            }
            None => {
                log::info!(
                    "Update group({}) verify ok, from {:?}/{:?}, to {}/{}",
                    group_id,
                    latest_group.map(|group| group.version()),
                    self.prev_blob_id(),
                    self.version(),
                    self.to_blob().desc().object_id()
                );
                Ok(())
            }
        }
    }
}
