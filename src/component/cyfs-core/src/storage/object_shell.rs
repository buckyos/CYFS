use cyfs_base::{
    BodyContent, BuckyError, BuckyErrorCode, BuckyResult, DescContent, HashValue, NamedObjType,
    NamedObject, NamedObjectBase, NamedObjectBaseBuilder, NamedObjectBodyContext,
    NamedObjectBuilder, NamedObjectDesc, NamedObjectId, ObjectDesc, ObjectId, ObjectMutBody,
    ObjectSigns, ObjectType, ProtobufDecode, ProtobufEncode, ProtobufTransform, RawConvertTo,
    RawDecode, RawEncode, RawEncodePurpose, RawEncodeWithContext, Signature, SubDescNone,
    OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF,
};
use serde::Serialize;
use sha2::Digest;

use crate::CoreObjectType;

#[derive(Debug, Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::ObjectShellDescContent)]
struct ObjectShellDescContent {
    is_object_id_only: bool,
    is_desc_sign_fix: bool,
    is_body_sign_fix: bool,
    is_nonce_fix: bool,
    fix_content_hash: HashValue,
}

impl DescContent for ObjectShellDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::ObjectShell as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::ObjectShellBodyContent)]
struct ObjectShellBodyContent {
    desc: Vec<u8>,
    body: Option<Vec<u8>>,
    desc_signatures: Option<Vec<u8>>,
    body_signatures: Option<Vec<u8>>,
    nonce: Option<Vec<u8>>,
}

impl BodyContent for ObjectShellBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl ObjectShellBodyContent {
    fn hash(&self, flags: &ObjectShellFlags) -> HashValue {
        let occupy = [0u8; 0];
        let occupy = occupy.raw_encode_to_buffer().unwrap();

        let mut sha256 = sha2::Sha256::new();
        sha256.input(self.desc.as_slice());
        sha256.input(
            self.body
                .as_ref()
                .map_or(occupy.as_slice(), |v| v.as_slice()),
        );

        if flags.is_desc_sign_fix {
            sha256.input(
                self.desc_signatures
                    .as_ref()
                    .map_or(occupy.as_slice(), |v| v.as_slice()),
            );
        }
        if flags.is_body_sign_fix {
            sha256.input(
                self.body_signatures
                    .as_ref()
                    .map_or(occupy.as_slice(), |v| v.as_slice()),
            );
        }
        if flags.is_nonce_fix {
            sha256.input(
                self.nonce
                    .as_ref()
                    .map_or(occupy.as_slice(), |v| v.as_slice()),
            );
        }
        HashValue::from(sha256.result())
    }
}

type ObjectShellType = NamedObjType<ObjectShellDescContent, ObjectShellBodyContent>;
type ObjectShellBuilder = NamedObjectBuilder<ObjectShellDescContent, ObjectShellBodyContent>;
type ObjectShellDesc = NamedObjectDesc<ObjectShellDescContent>;

type ObjectShellId = NamedObjectId<ObjectShellType>;
type ObjectShellStorage = NamedObjectBase<ObjectShellType>;

trait ObjectShellStorageObject {
    fn shell_id(&self) -> ObjectId;
    fn check_hash(&self) -> bool;
    fn hash(&self) -> Option<HashValue>;
    fn flags(&self) -> ObjectShellFlags;
}
impl ObjectShellStorageObject for ObjectShellStorage {
    fn shell_id(&self) -> ObjectId {
        self.desc().calculate_id()
    }

    fn check_hash(&self) -> bool {
        self.hash().as_ref().map_or(false, |hash| {
            &self.desc().content().fix_content_hash == hash
        })
    }

    fn hash(&self) -> Option<HashValue> {
        self.body()
            .as_ref()
            .map(|body| body.content().hash(&self.flags()))
    }

    fn flags(&self) -> ObjectShellFlags {
        let desc = self.desc().content();
        ObjectShellFlags {
            is_object_id_only: desc.is_object_id_only,
            is_desc_sign_fix: desc.is_desc_sign_fix,
            is_body_sign_fix: desc.is_body_sign_fix,
            is_nonce_fix: desc.is_nonce_fix,
        }
    }
}

#[derive(Copy, Clone)]
pub struct ObjectShellFlags {
    is_object_id_only: bool,
    is_desc_sign_fix: bool,
    is_body_sign_fix: bool,
    is_nonce_fix: bool,
}

impl ObjectShellFlags {
    pub fn object_id_only(&self) -> bool {
        self.is_object_id_only
    }

    pub fn desc_sign_fix(&self) -> bool {
        self.is_desc_sign_fix
    }

    pub fn body_sign_fix(&self) -> bool {
        self.is_body_sign_fix
    }

    pub fn nonce_fix(&self) -> bool {
        self.is_nonce_fix
    }
}

pub const OBJECT_SHELL_ALL_FREEDOM_WITH_FULL_DESC: ObjectShellFlags = ObjectShellFlags {
    is_object_id_only: false,
    is_desc_sign_fix: false,
    is_body_sign_fix: false,
    is_nonce_fix: false,
};

pub struct ObjectShellFlagsBuilder {
    flags: ObjectShellFlags,
}

impl ObjectShellFlagsBuilder {
    pub fn new() -> Self {
        Self {
            flags: OBJECT_SHELL_ALL_FREEDOM_WITH_FULL_DESC,
        }
    }

    pub fn build(&self) -> ObjectShellFlags {
        self.flags
    }

    pub fn object_id_only(&mut self, is_only: bool) -> &mut Self {
        self.flags.is_object_id_only = is_only;
        self
    }

    pub fn desc_sign_fix(&mut self, is_fix: bool) -> &mut Self {
        self.flags.is_desc_sign_fix = is_fix;
        self
    }

    pub fn body_sign_fix(&mut self, is_fix: bool) -> &mut Self {
        self.flags.is_body_sign_fix = is_fix;
        self
    }

    pub fn nonce_fix(&mut self, is_fix: bool) -> &mut Self {
        self.flags.is_nonce_fix = is_fix;
        self
    }
}

#[derive(Clone)]
enum ShelledDesc<D: ObjectDesc + Sync + Send + Clone> {
    ObjectId(ObjectId),
    Desc(D),
}

#[derive(Clone)]
pub struct ObjectShell<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
    O::DescType: Clone,
{
    desc: ShelledDesc<O::DescType>,
    body: Option<ObjectMutBody<O::ContentType, O>>,
    signs: ObjectSigns,
    nonce: Option<u128>,
    flags: ObjectShellFlags,
}

impl<O> ObjectShell<O>
where
    O: ObjectType,
    O::ContentType: BodyContent + RawEncode + for<'de> RawDecode<'de> + Clone,
    O::DescType: RawEncode + for<'de> RawDecode<'de> + Clone,
{
    pub fn from_object<NO>(raw: &NO, flags: ObjectShellFlags) -> Self
    where
        NO: NamedObject<O> + RawEncode + for<'local> RawDecode<'local> + Clone,
    {
        Self {
            flags,
            desc: ShelledDesc::Desc(raw.desc().clone()),
            body: raw.body().clone(),
            signs: raw.signs().clone(),
            nonce: raw.nonce().clone(),
        }
    }

    pub fn shell_id(&self) -> ObjectId {
        self.to_storage().shell_id()
    }

    pub fn flags(&self) -> &ObjectShellFlags {
        &self.flags
    }

    pub fn with_full_desc(&self) -> bool {
        match self.desc {
            ShelledDesc::ObjectId(_) => false,
            ShelledDesc::Desc(_) => true,
        }
    }

    pub fn body(&self) -> &Option<ObjectMutBody<O::ContentType, O>> {
        &self.body
    }
    // update the raw object
    pub fn body_mut(&mut self) -> &mut Option<ObjectMutBody<O::ContentType, O>> {
        &mut self.body
    }

    pub fn signs(&self) -> &ObjectSigns {
        &self.signs
    }

    // update the signatures
    pub fn signs_mut(&mut self) -> &mut ObjectSigns {
        &mut self.signs
    }

    pub fn nonce(&self) -> &Option<u128> {
        &self.nonce
    }

    // update the nonce
    pub fn nonce_mut(&mut self) -> &mut Option<u128> {
        &mut self.nonce
    }

    pub fn try_into_object(
        mut self,
        desc: Option<&O::DescType>,
    ) -> BuckyResult<NamedObjectBase<O>> {
        let body = self.body.take();
        let mut signs = ObjectSigns::default();
        std::mem::swap(&mut signs, &mut self.signs);
        let nonce = self.nonce.take();

        let desc = match self.desc {
            ShelledDesc::Desc(desc_inner) => {
                let id = desc_inner.object_id();
                if let Some(desc) = desc {
                    let obj_id_param = desc.object_id();
                    if obj_id_param != id {
                        let msg = format!(
                            "parameter desc({}) is not match with object-id({}) from desc.",
                            obj_id_param, id
                        );
                        log::error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
                    }
                }
                desc_inner
            }
            ShelledDesc::ObjectId(id) => match desc {
                Some(desc) => {
                    let obj_id_param = desc.object_id();
                    if obj_id_param == id {
                        desc.clone()
                    } else {
                        let msg = format!(
                            "parameter desc({}) is not match with object-id({}).",
                            obj_id_param, id
                        );
                        log::error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
                    }
                }
                None => {
                    let msg = format!(
                        "no desc stored in the shell, you should input it from parameters. object-id is {}",
                        id
                    );
                    log::error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            },
        };

        let builder = NamedObjectBaseBuilder::<O>::new(desc);
        let builder = if let Some(body) = body {
            builder.body(body)
        } else {
            builder
        };
        let builder = builder.signs(signs);
        let obj = if let Some(nonce) = nonce {
            builder.nonce(nonce).build()
        } else {
            builder.build()
        };

        Ok(obj)
    }

    fn from_storage(storage: &ObjectShellStorage) -> BuckyResult<Self> {
        if !storage.check_hash() {
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                "Hash does not match with raw object",
            ));
        }

        let flags = storage.flags();

        let storage_body = storage.body().as_ref().unwrap().content();

        let nonce = match storage_body.nonce.as_ref() {
            Some(nonce_buf) => {
                if nonce_buf.len() != 16 {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "nonce should be a u128.",
                    ));
                }
                let mut nonce = [0; 16];
                nonce.clone_from_slice(nonce_buf.as_slice());
                Some(u128::from_be_bytes(nonce))
            }
            None => None,
        };

        let desc = if flags.is_object_id_only {
            let (id, remain) = ObjectId::raw_decode(storage_body.desc.as_slice())?;
            assert_eq!(remain.len(), 0);
            ShelledDesc::ObjectId(id)
        } else {
            let (desc, remain) = O::DescType::raw_decode(storage_body.desc.as_slice())?;
            assert_eq!(remain.len(), 0);
            ShelledDesc::Desc(desc)
        };

        let body = match storage_body.body.as_ref() {
            Some(body) => {
                let (body, remain) =
                    ObjectMutBody::<O::ContentType, O>::raw_decode(body.as_slice())?;
                assert_eq!(remain.len(), 0);
                Some(body)
            }
            None => None,
        };

        let mut signs = ObjectSigns::default();
        if let Some(desc_signatures) = storage_body.desc_signatures.as_ref() {
            let (desc_signatures, remain) =
                Vec::<Signature>::raw_decode(desc_signatures.as_slice())?;
            assert_eq!(remain.len(), 0);
            for sign in desc_signatures {
                signs.push_desc_sign(sign);
            }
        };

        if let Some(body_signatures) = storage_body.body_signatures.as_ref() {
            let (body_signatures, remain) =
                Vec::<Signature>::raw_decode(body_signatures.as_slice())?;
            assert_eq!(remain.len(), 0);
            for sign in body_signatures {
                signs.push_body_sign(sign);
            }
        };

        Ok(Self {
            desc,
            body,
            signs,
            nonce,
            flags,
        })
    }

    fn to_storage(&self) -> ObjectShellStorage {
        let desc = match &self.desc {
            ShelledDesc::ObjectId(obj_id) => {
                assert!(self.flags.is_object_id_only);
                obj_id.to_vec().expect("encode desc as object-id failed.")
            }
            ShelledDesc::Desc(desc) => {
                if self.flags.is_object_id_only {
                    desc.object_id()
                        .to_vec()
                        .expect("encode desc as object-id from desc failed.")
                } else {
                    desc.to_vec().expect("encode desc as desc failed.")
                }
            }
        };

        let body = match self.body.as_ref() {
            Some(body) => {
                let mut ctx = NamedObjectBodyContext::new();
                let size = body
                    .raw_measure_with_context(&mut ctx, &None)
                    .expect("measure body failed.");
                let mut body_buf = vec![];
                body_buf.resize(size, 0u8);
                let remain = body
                    .raw_encode_with_context(body_buf.as_mut(), &mut ctx, &None)
                    .expect("encode body failed.");
                assert_eq!(remain.len(), 0);
                Some(body_buf)
            }
            None => None,
        };

        let desc_signatures = match self.signs.desc_signs().as_ref() {
            Some(desc_signatures) => Some(
                desc_signatures
                    .to_vec()
                    .expect("encode desc-signatures failed."),
            ),
            None => None,
        };

        let body_signatures = match self.signs.body_signs().as_ref() {
            Some(body_signatures) => Some(
                body_signatures
                    .to_vec()
                    .expect("encode body-signatures failed."),
            ),
            None => None,
        };

        let nonce = self.nonce.clone();

        let storage_body = ObjectShellBodyContent {
            desc,
            body,
            desc_signatures,
            body_signatures,
            nonce: nonce.map(|n| Vec::from(n.to_be_bytes())),
        };

        let hash = storage_body.hash(&self.flags);

        let storage_desc = ObjectShellDescContent {
            is_object_id_only: self.flags.is_object_id_only,
            is_desc_sign_fix: self.flags.is_desc_sign_fix,
            is_body_sign_fix: self.flags.is_body_sign_fix,
            is_nonce_fix: self.flags.is_nonce_fix,
            fix_content_hash: hash,
        };

        let shell = ObjectShellBuilder::new(storage_desc, storage_body)
            .no_create_time()
            .build();
        shell
    }
}

impl<O> RawEncode for ObjectShell<O>
where
    O: ObjectType,
    O::ContentType: BodyContent + RawEncode + for<'de> RawDecode<'de> + Clone,
    O::DescType: RawEncode + for<'de> RawDecode<'de> + Clone,
{
    fn raw_measure(&self, purpose: &Option<cyfs_base::RawEncodePurpose>) -> BuckyResult<usize> {
        self.to_storage().raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<cyfs_base::RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        self.to_storage().raw_encode(buf, purpose)
    }
}

impl<'de, O> RawDecode<'de> for ObjectShell<O>
where
    O: ObjectType,
    O::ContentType: BodyContent + RawEncode + for<'de1> RawDecode<'de1> + Clone,
    O::DescType: RawEncode + for<'de1> RawDecode<'de1> + Clone,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (storage, remain) = ObjectShellStorage::raw_decode(buf)?;
        Self::from_storage(&storage).map(|o| (o, remain))
    }
}

#[cfg(test)]
mod object_shell_test {
    use crate::{ObjectShell, Text, TextObj, OBJECT_SHELL_ALL_FREEDOM_WITH_FULL_DESC};

    #[test]
    fn test() {
        let txt_v1 = Text::create("txt-storage", "header", "v1");
        // shell with full-desc, desc-signatures freedom, body-signatures freedom, nonce freedom.
        let txt_shell_v1 =
            ObjectShell::from_object::<Text>(&txt_v1, OBJECT_SHELL_ALL_FREEDOM_WITH_FULL_DESC);
        let txt_v1_from_shell = txt_shell_v1
            .try_into_object(None)
            .expect("recover text from shell failed.");
        let shell_id_v1 = txt_shell_v1.shell_id();
        assert_eq!(txt_v1_from_shell.id(), txt_v1.id());
        assert_eq!(txt_v1_from_shell.header(), txt_v1.header());
        assert_eq!(txt_v1_from_shell.value(), txt_v1.value());

        // shell-id changed when the body updated.
        let mut txt_shell_v2 = txt_shell_v1.clone();
        *txt_shell_v2.body_mut().unwrap().content().value_mut() = "v2".to_string();
        let txt_v2_from_shell = txt_shell_v2
            .try_into_object(None)
            .expect("recover text from shell failed.");
        let shell_id_v2 = txt_shell_v2.shell_id();
        assert_eq!(txt_v2_from_shell.id(), txt_v1.id());
        assert_eq!(txt_v2_from_shell.header(), txt_v1.header());
        assert_eq!(txt_v2_from_shell.value(), "v2");
        assert_ne!(shell_id_v1, shell_id_v2);

        // shell-id not changed when the nonce updated.
        let mut txt_shell_v2_nonce = txt_shell_v2.clone();
        *txt_shell_v2_nonce.nonce_mut() = Some(1);
        let txt_v2_nonce_from_shell = txt_shell_v2_nonce
            .try_into_object(None)
            .expect("recover text from shell failed.");
        let shell_id_v2_nonce = txt_shell_v2_nonce.shell_id();
        assert_eq!(txt_v2_nonce_from_shell.id(), txt_v1.id());
        assert_eq!(txt_v2_nonce_from_shell.header(), txt_v1.header());
        assert_eq!(txt_v2_nonce_from_shell.value(), txt_v2_from_shell.value());
        assert_ne!(shell_id_v2_nonce, shell_id_v2);
    }
}
