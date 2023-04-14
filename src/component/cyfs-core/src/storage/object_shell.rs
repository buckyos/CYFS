use std::marker::PhantomData;

use cyfs_base::{
    BodyContent, BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId,
    ObjectType, RawDecode, RawEncode, Signature,
};

use crate::{Storage, StorageObj};

const OBJECT_SHELL_FLAGS_FREEDOM_DESC_SIGNATURE: u8 = 0b_1;
const OBJECT_SHELL_FLAGS_FREEDOM_BODY_SIGNATURE: u8 = 0b_10;
const OBJECT_SHELL_FLAGS_FREEDOM_NONCE: u8 = 0b_100;
const OBJECT_SHELL_FLAGS_EXT: u8 = 0b_10000000;

#[derive(Copy, Clone)]
pub struct ObjectShellFlags {
    flags: u8,
}

pub const OBJECT_SHELL_ALL_FREEDOM: ObjectShellFlags = ObjectShellFlags {
    flags: OBJECT_SHELL_FLAGS_FREEDOM_DESC_SIGNATURE
        | OBJECT_SHELL_FLAGS_FREEDOM_BODY_SIGNATURE
        | OBJECT_SHELL_FLAGS_FREEDOM_NONCE,
};

impl ObjectShellFlags {
    fn is_desc_sign_freedom(&self) -> bool {
        self.flags & OBJECT_SHELL_FLAGS_FREEDOM_DESC_SIGNATURE != 0
    }

    fn is_body_sign_freedom(&self) -> bool {
        self.flags & OBJECT_SHELL_FLAGS_FREEDOM_BODY_SIGNATURE != 0
    }

    fn is_nonce_freedom(&self) -> bool {
        self.flags & OBJECT_SHELL_FLAGS_FREEDOM_NONCE != 0
    }
}

pub struct ObjectShellFlagsBuilder {
    flags: u8,
}

impl ObjectShellFlagsBuilder {
    pub fn new() -> Self {
        Self { flags: 0 }
    }

    pub fn build(&self) -> ObjectShellFlags {
        ObjectShellFlags { flags: self.flags }
    }

    pub fn freedom_desc_signature(&mut self) -> &mut Self {
        self.flags |= OBJECT_SHELL_FLAGS_FREEDOM_DESC_SIGNATURE;
        self
    }

    pub fn freedom_body_signature(&mut self) -> &mut Self {
        self.flags |= OBJECT_SHELL_FLAGS_FREEDOM_BODY_SIGNATURE;
        self
    }

    pub fn freedom_nonce(&mut self) -> &mut Self {
        self.flags |= OBJECT_SHELL_FLAGS_FREEDOM_NONCE;
        self
    }
}

pub struct ObjectShell<O, OT> {
    raw: O,
    flags: ObjectShellFlags,
    phantom: PhantomData<OT>,
}

impl<O, OT> ObjectShell<O, OT>
where
    O: NamedObject<OT> + RawEncode + for<'local> RawDecode<'local> + Clone, // TODO: how to support other parameter against `O` for `NamedObject`
    OT: ObjectType,
    OT::ContentType: BodyContent,
{
    pub fn from_storage(storage: &Storage) -> BuckyResult<Self> {
        if storage.check_hash() != Some(true) {
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                "Hash does not match with raw object",
            ));
        }

        let value = storage.value().as_slice();
        if value.len() == 0 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "empty for object shell",
            ));
        }

        let (freedom_flags, unfreedom) = value.split_at(1);

        let freedom_flags = ObjectShellFlags {
            flags: *freedom_flags.get(0).unwrap(),
        };

        let (mut raw, remain) = O::raw_decode(unfreedom)?;
        assert_eq!(remain.len(), 0);

        match storage.freedom_attachment().as_ref() {
            Some(freedom) if freedom.len() > 0 => {
                let (exist_field_flags, freedom) = freedom.split_at(1);
                let exist_field_flags = ObjectShellFlags {
                    flags: *exist_field_flags.get(0).unwrap(),
                };

                let mut buf = freedom;

                if freedom_flags.is_desc_sign_freedom() && exist_field_flags.is_desc_sign_freedom()
                {
                    let (signs, remain) = Vec::<Signature>::raw_decode(buf)?;
                    buf = remain;
                    for sign in signs {
                        raw.signs_mut().push_desc_sign(sign);
                    }
                }

                if freedom_flags.is_body_sign_freedom() && exist_field_flags.is_body_sign_freedom()
                {
                    let (signs, remain) = Vec::<Signature>::raw_decode(buf)?;
                    buf = remain;
                    for sign in signs {
                        raw.signs_mut().push_body_sign(sign);
                    }
                }

                if freedom_flags.is_nonce_freedom() && exist_field_flags.is_nonce_freedom() {
                    let (nonce, remain) = u128::raw_decode(buf)?;
                    buf = remain;
                    unreachable!("nonce is not supported currently for the NamedObject::set_nonce is not exported.");
                    // raw.set_nonce()
                }
                assert_eq!(buf.len(), 0);
            }
            _ => {}
        }

        Ok(Self {
            raw,
            flags: freedom_flags,
            phantom: PhantomData,
        })
    }

    pub fn to_storage(&self) -> Storage {
        let mut const_raw = self.raw.clone();
        let max_buf_size = const_raw
            .raw_measure(&None)
            .expect("encode measure faield for object-shell");

        let mut freedom_buf = vec![0; max_buf_size + 1];
        let mut exist_freedom_field_flags = ObjectShellFlagsBuilder::new();
        let (freedom_flag_buf, mut freedom_remain) = freedom_buf.split_at_mut(1);

        if self.flags.is_desc_sign_freedom() {
            let desc_signs = const_raw.signs_mut().desc_signs();
            if let Some(signs) = desc_signs {
                if signs.len() > 0 {
                    freedom_remain = signs
                        .raw_encode(freedom_remain, &None)
                        .expect("encode desc signature for object-shell failed.");
                    exist_freedom_field_flags.freedom_desc_signature();
                    const_raw.signs_mut().clear_desc_signs();
                }
            }
        }

        if self.flags.is_body_sign_freedom() {
            let body_signs = const_raw.signs_mut().body_signs();
            if let Some(signs) = body_signs {
                if signs.len() > 0 {
                    freedom_remain = signs
                        .raw_encode(freedom_remain, &None)
                        .expect("encode body signature for object-shell failed.");
                    exist_freedom_field_flags.freedom_body_signature();
                    const_raw.signs_mut().clear_body_signs();
                }
            }
        }

        if self.flags.is_nonce_freedom() {
            let nonce = const_raw.nonce();
            if let Some(nonce) = nonce.as_ref() {
                freedom_remain = nonce
                    .raw_encode(freedom_remain, &None)
                    .expect("encode nonce for object-shell failed.");
                exist_freedom_field_flags.freedom_nonce();

                unreachable!(
                    "nonce is not supported currently for the NamedObject::set_nonce is not exported."
                );
                // const_raw.set_nonce(None);
            }
        }

        let freedom_attachment = if freedom_remain.len() < max_buf_size {
            *freedom_flag_buf.get_mut(0).unwrap() = exist_freedom_field_flags.flags;
            let len = max_buf_size + 1 - freedom_remain.len();
            unsafe {
                freedom_buf.set_len(len);
            }
            Some(freedom_buf)
        } else {
            None
        };

        let mut value = vec![0; max_buf_size + 1];
        *value.get_mut(0).unwrap() = self.flags.flags;
        let remain = const_raw
            .raw_encode(&mut value.as_mut_slice()[1..], &None)
            .unwrap();
        let len = max_buf_size + 1 - remain.len();
        unsafe {
            value.set_len(len);
        }

        Storage::create_with_hash_and_freedom(
            self.raw.desc().object_id().to_string().as_str(),
            value,
            freedom_attachment,
        )
    }

    pub fn shell_id(&self) -> ObjectId {
        self.to_storage().storage_id().object_id().clone()
    }

    pub fn from_object(raw: O, flags: ObjectShellFlags) -> Self {
        Self {
            raw,
            flags,
            phantom: PhantomData,
        }
    }

    pub fn as_ref(&self) -> &O {
        &self.raw
    }

    pub fn as_mut(&mut self) -> &mut O {
        // update the raw object
        &mut self.raw
    }
}

impl<O, OT> RawEncode for ObjectShell<O, OT>
where
    O: NamedObject<OT> + RawEncode + for<'de> RawDecode<'de> + Clone,
    OT: ObjectType,
    OT::ContentType: BodyContent,
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

impl<'de, O, OT> RawDecode<'de> for ObjectShell<O, OT>
where
    O: NamedObject<OT> + RawEncode + for<'local> RawDecode<'local> + Clone,
    OT: ObjectType,
    OT::ContentType: BodyContent,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (storage, remain) = Storage::raw_decode(buf)?;
        Self::from_storage(&storage).map(|o| (o, remain))
    }
}
