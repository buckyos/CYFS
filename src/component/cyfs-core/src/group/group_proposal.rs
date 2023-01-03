use crate::{CoreObjectType, GroupPropsalDecideParam, GroupRPath};
use async_trait::async_trait;
use cyfs_base::*;
use serde::Serialize;
use sha2::Digest;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::GroupProposalDescContent)]
pub struct GroupProposalDescContent {
    r_path: GroupRPath,
    method: String,
    params: Option<Vec<u8>>,

    meta_block_id: Option<ObjectId>,
    effective_begining: Option<u64>,
    effective_ending: Option<u64>,
}

impl DescContent for GroupProposalDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::GroupProposal as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("GroupProposalDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = Option<Area>;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone)]
pub struct GroupProposalSignature {
    signature: Signature,
    proponent_id: ObjectId,
    decide: Vec<u8>,
}

impl ProtobufTransform<&crate::codec::protos::group_proposal_body_content::Signature>
    for GroupProposalSignature
{
    fn transform(
        value: &crate::codec::protos::group_proposal_body_content::Signature,
    ) -> BuckyResult<Self> {
        Ok(Self {
            signature: Signature::raw_decode(value.signature.as_slice())?.0,
            proponent_id: ObjectId::raw_decode(value.proponent_id.as_slice())?.0,
            decide: value.decide.clone(),
        })
    }
}

impl ProtobufTransform<&GroupProposalSignature>
    for crate::codec::protos::group_proposal_body_content::Signature
{
    fn transform(value: &GroupProposalSignature) -> BuckyResult<Self> {
        Ok(Self {
            signature: value.signature.to_vec()?,
            proponent_id: value.proponent_id.to_vec()?,
            decide: value.decide.clone(),
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Default)]
#[cyfs_protobuf_type(crate::codec::protos::GroupProposalBodyContent)]
pub struct GroupProposalBodyContent {
    payload: Option<Vec<u8>>,

    decide_signatures: Vec<GroupProposalSignature>,
}

impl BodyContent for GroupProposalBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl ProtobufTransform<crate::codec::protos::GroupProposalBodyContent>
    for GroupProposalBodyContent
{
    fn transform(value: crate::codec::protos::GroupProposalBodyContent) -> BuckyResult<Self> {
        let mut decide_signatures = vec![];
        for sign in value.decide_signatures.as_slice() {
            decide_signatures.push(GroupProposalSignature::transform(sign)?);
        }

        Ok(Self {
            payload: value.payload,
            decide_signatures,
        })
    }
}

impl ProtobufTransform<&GroupProposalBodyContent>
    for crate::codec::protos::GroupProposalBodyContent
{
    fn transform(value: &GroupProposalBodyContent) -> BuckyResult<Self> {
        let mut decide_signatures = vec![];
        for sign in value.decide_signatures.as_slice() {
            decide_signatures.push(
                crate::codec::protos::group_proposal_body_content::Signature::transform(sign)?,
            );
        }

        Ok(Self {
            payload: value.payload.clone(),
            decide_signatures,
        })
    }
}

pub type GroupProposalType = NamedObjType<GroupProposalDescContent, GroupProposalBodyContent>;
pub type GroupProposalBuilder =
    NamedObjectBuilder<GroupProposalDescContent, GroupProposalBodyContent>;

pub type GroupProposalId = NamedObjectId<GroupProposalType>;
pub type GroupProposal = NamedObjectBase<GroupProposalType>;

impl GroupProposalDescContent {
    pub fn new(
        r_path: GroupRPath,
        method: String,
        params: Option<Vec<u8>>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> GroupProposalDescContent {
        Self {
            r_path,
            method,
            params,
            meta_block_id,
            effective_begining,
            effective_ending,
        }
    }
}

impl GroupProposalBodyContent {
    fn hash_decide_signature(
        proposal_id: &ObjectId,
        proponent_id: &ObjectId,
        decide: &[u8],
    ) -> HashValue {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(proposal_id.as_slice());
        sha256.input(proponent_id.as_slice());
        sha256.input(decide);
        sha256.result().into()
    }
}

#[async_trait]
pub trait GroupProposalObject {
    fn create(
        r_path: GroupRPath,
        method: String,
        params: Option<Vec<u8>>,
        payload: Option<Vec<u8>>,
        timestamp: Option<u64>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> GroupProposalBuilder;

    fn r_path(&self) -> &GroupRPath;
    fn method(&self) -> &str;
    fn params(&self) -> &Option<Vec<u8>>;
    fn params_hash(&self) -> BuckyResult<Option<HashValue>>;
    fn params_object_id(&self) -> BuckyResult<Option<ObjectId>>;
    fn meta_block_id(&self) -> &Option<ObjectId>;
    fn timestamp(&self) -> u64;
    fn effective_begining(&self) -> Option<u64>;
    fn effective_ending(&self) -> Option<u64>;

    fn payload(&self) -> &Option<Vec<u8>>;
    fn set_payload(&mut self, payload: Option<Vec<u8>>);

    async fn verify_member_decide(
        &self,
        member_id: &ObjectId,
        public_key: &PublicKey,
    ) -> BuckyResult<Vec<&[u8]>>;

    fn decided_members_no_verify(&self) -> &Vec<GroupProposalSignature>;

    async fn decide(
        &self,
        member_id: ObjectId,
        decide: Vec<u8>,
        private_key: &PrivateKey,
    ) -> BuckyResult<GroupPropsalDecideParam>;

    async fn verify_and_merge_decide(
        &mut self,
        decide: &GroupPropsalDecideParam,
        member_id: ObjectId,
        public_key: &PublicKey,
    ) -> BuckyResult<()>;
}

#[async_trait]
impl GroupProposalObject for GroupProposal {
    fn create(
        r_path: GroupRPath,
        method: String,
        params: Option<Vec<u8>>,
        payload: Option<Vec<u8>>,
        timestamp: Option<u64>,
        meta_block_id: Option<ObjectId>,
        effective_begining: Option<u64>,
        effective_ending: Option<u64>,
    ) -> GroupProposalBuilder {
        let desc = GroupProposalDescContent {
            r_path,
            method,
            params,
            meta_block_id,
            effective_begining,
            effective_ending,
        };

        GroupProposalBuilder::new(
            desc,
            GroupProposalBodyContent {
                payload,
                decide_signatures: vec![],
            },
        )
        .create_time(timestamp.map_or(bucky_time_now(), |t| t))
    }

    fn r_path(&self) -> &GroupRPath {
        &self.desc().content().r_path
    }

    fn method(&self) -> &str {
        self.desc().content().method.as_str()
    }

    fn params(&self) -> &Option<Vec<u8>> {
        &self.desc().content().params
    }

    fn params_hash(&self) -> BuckyResult<Option<HashValue>> {
        match &self.desc().content().params {
            Some(params) => {
                if params.len() != HASH_VALUE_LEN {
                    Err(BuckyError::new(
                        BuckyErrorCode::Unmatch,
                        format!(
                            "try parse GroupProposal.param as hash with error length: ${}",
                            params.len()
                        ),
                    ))
                } else {
                    Ok(Some(HashValue::from(params.as_slice())))
                }
            }
            None => Ok(None),
        }
    }

    fn params_object_id(&self) -> BuckyResult<Option<ObjectId>> {
        match &self.desc().content().params {
            Some(params) => {
                if params.len() != OBJECT_ID_LEN {
                    Err(BuckyError::new(
                        BuckyErrorCode::Unmatch,
                        format!(
                            "try parse GroupProposal.param as ObjectId with error length: ${}",
                            params.len()
                        ),
                    ))
                } else {
                    Ok(Some(ObjectId::raw_decode(params.as_slice())?.0))
                }
            }
            None => Ok(None),
        }
    }

    fn meta_block_id(&self) -> &Option<ObjectId> {
        &self.desc().content().meta_block_id
    }

    fn effective_begining(&self) -> Option<u64> {
        self.desc().content().effective_begining
    }

    fn effective_ending(&self) -> Option<u64> {
        self.desc().content().effective_ending
    }

    fn payload(&self) -> &Option<Vec<u8>> {
        &self.body().as_ref().unwrap().content().payload
    }

    fn set_payload(&mut self, payload: Option<Vec<u8>>) {
        self.body_mut().as_mut().unwrap().content_mut().payload = payload;
    }

    async fn verify_member_decide(
        &self,
        member_id: &ObjectId,
        public_key: &PublicKey,
    ) -> BuckyResult<Vec<&[u8]>> {
        let signs = self
            .body()
            .as_ref()
            .unwrap()
            .content()
            .decide_signatures
            .as_slice();

        let proposal_id = self.desc().object_id();
        let verifier = RsaCPUObjectVerifier::new(public_key.clone());

        let mut decides = vec![];

        for sign in signs {
            if &sign.proponent_id == member_id {
                let hash = GroupProposalBodyContent::hash_decide_signature(
                    &proposal_id,
                    &sign.proponent_id,
                    sign.decide.as_slice(),
                );

                if verifier.verify(hash.as_slice(), &sign.signature).await {
                    decides.push(sign.decide.as_slice());
                } else {
                    return Err(BuckyError::new(
                        BuckyErrorCode::InvalidSignature,
                        "invalid signature",
                    ));
                }
            }
        }

        Ok(decides)
    }

    fn decided_members_no_verify(&self) -> &Vec<GroupProposalSignature> {
        &self.body().as_ref().unwrap().content().decide_signatures
    }

    async fn decide(
        &self,
        member_id: ObjectId,
        decide: Vec<u8>,
        private_key: &PrivateKey,
    ) -> BuckyResult<GroupPropsalDecideParam> {
        let signs = &self.body().as_ref().unwrap().content().decide_signatures;

        if signs.iter().find(|s| s.proponent_id == member_id).is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::AlreadyExists,
                "duplicated decide",
            ));
        }

        let proposal_id = self.desc().object_id();

        let hash = GroupProposalBodyContent::hash_decide_signature(
            &proposal_id,
            &member_id,
            decide.as_slice(),
        );

        let signer = RsaCPUObjectSigner::new(private_key.public(), private_key.clone());
        let sign = signer
            .sign(hash.as_slice(), &SignatureSource::RefIndex(0))
            .await?;

        Ok(GroupPropsalDecideParam::new(sign, proposal_id, decide))
    }

    async fn verify_and_merge_decide(
        &mut self,
        decide: &GroupPropsalDecideParam,
        member_id: ObjectId,
        public_key: &PublicKey,
    ) -> BuckyResult<()> {
        let proposal_id = self.desc().object_id();

        if decide.proposal_id() != &proposal_id {
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                format!(
                    "proposal id not match for decide signature: {}/{}",
                    proposal_id,
                    decide.proposal_id()
                ),
            ));
        }

        let hash = GroupProposalBodyContent::hash_decide_signature(
            &proposal_id,
            &member_id,
            decide.decide(),
        );

        let verifier = RsaCPUObjectVerifier::new(public_key.clone());
        if verifier.verify(hash.as_slice(), decide.signature()).await {
            let signs = &mut self
                .body_mut()
                .as_mut()
                .unwrap()
                .content_mut()
                .decide_signatures;
            for exist in signs.iter() {
                if &exist.proponent_id == &member_id && exist.decide == decide.decide() {
                    return Ok(());
                }
            }

            signs.push(GroupProposalSignature {
                signature: decide.signature().clone(),
                proponent_id: member_id,
                decide: Vec::from(decide.decide()),
            });

            Ok(())
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "invalid signature",
            ))
        }
    }
}
#[cfg(test)]
mod test {
    use super::{GroupProposal, GroupProposalObject};
    use cyfs_base::*;

    #[async_std::test]
    async fn create_group_proposal() {
        // let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        // let secret2 = PrivateKey::generate_rsa(1024).unwrap();
        // let people1 = People::new(None, vec![], secret1.public(), None, None, None).build();
        // let people1_id = people1.desc().people_id();
        // let people2 = People::new(None, vec![], secret2.public(), None, None, None).build();
        // let _people2_id = people2.desc().people_id();

        // let g1 = GroupRPath::create(
        //     people1_id.object_id().to_owned(),
        //     people1_id.object_id().to_owned(),
        //     people1_id.to_string(),
        // );

        // let buf = g1.to_vec().unwrap();
        // let add2 = GroupRPath::clone_from_slice(&buf).unwrap();
        // let any = AnyNamedObject::clone_from_slice(&buf).unwrap();
        // assert_eq!(g1.desc().calculate_id(), add2.desc().calculate_id());
        // assert_eq!(g1.desc().calculate_id(), any.calculate_id());
    }
}
