use crate::*;

#[derive(Clone, Debug)]
pub enum StandardObject {
    Device(Device),
    People(People),
    AppGroup(AppGroup),
    UnionAccount(UnionAccount),
    ChunkId(ChunkId),
    File(File),
    Dir(Dir),
    Diff(Diff),
    ProofOfService(ProofOfService),
    Tx(Tx),
    Action(Action),
    ObjectMap(ObjectMap),
    Contract(Contract),
    Group(Group),
}

#[macro_export]
macro_rules! match_standard_obj {
    ($on:ident, $o:ident, $body:tt, $chunk_id:ident, $chunk_body:tt) => {
        match $on {
            StandardObject::Device($o) => $body,
            StandardObject::People($o) => $body,
            StandardObject::Group($o) => $body,
            StandardObject::AppGroup($o) => $body,
            StandardObject::UnionAccount($o) => $body,
            StandardObject::ChunkId($chunk_id) => $chunk_body,
            StandardObject::File($o) => $body,
            StandardObject::Dir($o) => $body,
            StandardObject::Diff($o) => $body,
            StandardObject::ProofOfService($o) => $body,
            StandardObject::Tx($o) => $body,
            StandardObject::Action($o) => $body,
            StandardObject::ObjectMap($o) => $body,
            StandardObject::Contract($o) => $body,
        }
    };
}

macro_rules! match_standard_owner_obj {
    ($on:ident, $o:ident, $body:tt, $other_body:tt) => {
        match $on {
            StandardObject::Device($o) => $body,
            StandardObject::People($o) => $body,
            StandardObject::Contract($o) => $body,
            StandardObject::File($o) => $body,
            StandardObject::Dir($o) => $body,
            StandardObject::Diff($o) => $body,
            StandardObject::ProofOfService($o) => $body,
            StandardObject::Action($o) => $body,
            StandardObject::ObjectMap($o) => $body,
            _ => $other_body,
        }
    };
}

macro_rules! match_standard_pubkey_obj {
    ($on:ident, $o:ident, $body:tt, $other_body:tt) => {
        match $on {
            StandardObject::Device($o) => $body,
            StandardObject::People($o) => $body,
            // StandardObject::Group($o) => $body,
            _ => $other_body,
        }
    };
}

macro_rules! match_standard_author_obj {
    ($on:ident, $o:ident, $body:tt, $other_body:tt) => {
        match $on {
            StandardObject::File($o) => $body,
            StandardObject::Contract($o) => $body,
            _ => $other_body,
        }
    };
}

macro_rules! match_standard_ood_list_obj {
    ($on:ident, $o:ident, $body:tt, $other_body:tt) => {
        match $on {
            StandardObject::Group($o) => $body,
            StandardObject::People($o) => $body,
            _ => $other_body,
        }
    };
}

macro_rules! match_standard_ood_work_mode_obj {
    ($on:ident, $o:ident, $body:tt, $other_body:tt) => {
        match $on {
            StandardObject::People($o) => $body,
            _ => $other_body,
        }
    };
}

impl StandardObject {
    pub fn calculate_id(&self) -> ObjectId {
        match_standard_obj!(self, o, { o.desc().calculate_id() }, chunk_id, {
            chunk_id.object_id()
        })
    }

    pub fn obj_type(&self) -> BuckyResult<u16> {
        match_standard_obj!(self, o, { Ok(o.desc().obj_type()) }, _chunk_id, {
            Ok(ObjectTypeCode::Chunk.to_u16())
        })
    }

    pub fn obj_type_code(&self) -> ObjectTypeCode {
        match_standard_obj!(self, o, { o.desc().obj_type_code() }, _chunk_id, {
            ObjectTypeCode::Chunk
        })
    }

    pub fn dec_id(&self) -> &Option<ObjectId> {
        match_standard_obj!(self, o, { o.desc().dec_id() }, _chunk_id, { &None })
    }

    pub fn owner(&self) -> &Option<ObjectId> {
        match_standard_owner_obj!(self, o, { o.desc().owner() }, { &None })
    }

    pub fn prev(&self) -> &Option<ObjectId> {
        match_standard_owner_obj!(self, o, { o.desc().prev() }, { &None })
    }

    pub fn public_key(&self) -> Option<PublicKeyRef> {
        match_standard_pubkey_obj!(self, o, { o.desc().public_key_ref() }, { None })
    }

    pub fn author(&self) -> &Option<ObjectId> {
        match_standard_author_obj!(self, o, { o.desc().author() }, { &None })
    }

    pub fn ood_list(&self) -> BuckyResult<&Vec<DeviceId>> {
        match_standard_ood_list_obj!(
            self,
            o,
            {
                let b = o.body().as_ref();
                if b.is_none() {
                    Err(BuckyError::new(BuckyErrorCode::NotFound, "missing body"))
                } else {
                    let b = b.unwrap();
                    Ok(b.content().ood_list())
                }
            },
            {
                Err(BuckyError::new(
                    BuckyErrorCode::NotSupport,
                    "ood_list not support",
                ))
            }
        )
    }

    pub fn ood_work_mode(&self) -> BuckyResult<OODWorkMode> {
        match_standard_ood_work_mode_obj!(
            self,
            o,
            {
                let b = o.body().as_ref();
                if b.is_none() {
                    Err(BuckyError::new(BuckyErrorCode::NotFound, "missing body"))
                } else {
                    let b = b.unwrap();
                    Ok(b.content().ood_work_mode())
                }
            },
            {
                Err(BuckyError::new(
                    BuckyErrorCode::NotSupport,
                    "ood_work_mode not support",
                ))
            }
        )
    }

    pub fn set_body_expect(&mut self, other: &Self) {
        match self {
            Self::Device(o) => match other {
                Self::Device(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::People(o) => match other {
                Self::People(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::Group(o) => match other {
                Self::Group(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::AppGroup(o) => match other {
                Self::AppGroup(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::UnionAccount(o) => match other {
                Self::UnionAccount(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::File(o) => match other {
                Self::File(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::Dir(o) => match other {
                Self::Dir(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::Diff(o) => match other {
                Self::Diff(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::ProofOfService(o) => match other {
                Self::ProofOfService(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::Tx(o) => match other {
                Self::Tx(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::Action(o) => match other {
                Self::Action(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::ObjectMap(o) => match other {
                Self::ObjectMap(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::Contract(o) => match other {
                Self::Contract(other) => {
                    *o.body_mut() = other.body().clone();
                }
                _ => unreachable!(),
            },
            Self::ChunkId(_) => {
                unreachable!();
            }
        }
    }
}

impl RawEncode for StandardObject {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self {
            StandardObject::Device(o) => o.raw_measure(purpose),
            StandardObject::People(o) => o.raw_measure(purpose),
            StandardObject::Group(o) => o.raw_measure(purpose),
            StandardObject::AppGroup(o) => o.raw_measure(purpose),
            StandardObject::UnionAccount(o) => o.raw_measure(purpose),
            StandardObject::ChunkId(o) => o.raw_measure(purpose),
            StandardObject::File(o) => o.raw_measure(purpose),
            StandardObject::Dir(o) => o.raw_measure(purpose),
            StandardObject::Diff(o) => o.raw_measure(purpose),
            StandardObject::ProofOfService(o) => o.raw_measure(purpose),
            StandardObject::Tx(o) => o.raw_measure(purpose),
            StandardObject::Action(o) => o.raw_measure(purpose),
            StandardObject::ObjectMap(o) => o.raw_measure(purpose),
            StandardObject::Contract(o) => o.raw_measure(purpose),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            StandardObject::Device(o) => o.raw_encode(buf, purpose),
            StandardObject::People(o) => o.raw_encode(buf, purpose),
            StandardObject::Group(o) => o.raw_encode(buf, purpose),
            StandardObject::AppGroup(o) => o.raw_encode(buf, purpose),
            StandardObject::UnionAccount(o) => o.raw_encode(buf, purpose),
            StandardObject::ChunkId(o) => o.raw_encode(buf, purpose),
            StandardObject::File(o) => o.raw_encode(buf, purpose),
            StandardObject::Dir(o) => o.raw_encode(buf, purpose),
            StandardObject::Diff(o) => o.raw_encode(buf, purpose),
            StandardObject::ProofOfService(o) => o.raw_encode(buf, purpose),
            StandardObject::Tx(o) => o.raw_encode(buf, purpose),
            StandardObject::Action(o) => o.raw_encode(buf, purpose),
            StandardObject::ObjectMap(o) => o.raw_encode(buf, purpose),
            StandardObject::Contract(o) => o.raw_encode(buf, purpose),
        }
    }
}

// 通用的单个对象解码器
impl<'de> RawDecode<'de> for StandardObject {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ctx, _) = NamedObjectContext::raw_decode(buf).map_err(|e| {
            log::error!("StandardObject::raw_decode/NamedObjectContext error:{}", e);
            e
        })?;

        match ctx.obj_type_code() {
            ObjectTypeCode::Device => {
                Device::raw_decode(buf).map(|(obj, buf)| (StandardObject::Device(obj), buf))
            }
            ObjectTypeCode::People => {
                People::raw_decode(buf).map(|(obj, buf)| (StandardObject::People(obj), buf))
            }
            ObjectTypeCode::Group => {
                Group::raw_decode(buf).map(|(obj, buf)| (StandardObject::Group(obj), buf))
            }
            ObjectTypeCode::AppGroup => {
                AppGroup::raw_decode(buf).map(|(obj, buf)| (StandardObject::AppGroup(obj), buf))
            }
            ObjectTypeCode::UnionAccount => UnionAccount::raw_decode(buf)
                .map(|(obj, buf)| (StandardObject::UnionAccount(obj), buf)),
            ObjectTypeCode::Chunk => {
                ChunkId::raw_decode(buf).map(|(obj, buf)| (StandardObject::ChunkId(obj), buf))
            }
            ObjectTypeCode::File => {
                File::raw_decode(buf).map(|(obj, buf)| (StandardObject::File(obj), buf))
            }
            ObjectTypeCode::Dir => {
                Dir::raw_decode(buf).map(|(obj, buf)| (StandardObject::Dir(obj), buf))
            }
            ObjectTypeCode::Diff => {
                Diff::raw_decode(buf).map(|(obj, buf)| (StandardObject::Diff(obj), buf))
            }
            ObjectTypeCode::ProofOfService => ProofOfService::raw_decode(buf)
                .map(|(obj, buf)| (StandardObject::ProofOfService(obj), buf)),
            ObjectTypeCode::Tx => {
                Tx::raw_decode(buf).map(|(obj, buf)| (StandardObject::Tx(obj), buf))
            }
            ObjectTypeCode::Action => {
                Action::raw_decode(buf).map(|(obj, buf)| (StandardObject::Action(obj), buf))
            }
            ObjectTypeCode::ObjectMap => {
                ObjectMap::raw_decode(buf).map(|(obj, buf)| (StandardObject::ObjectMap(obj), buf))
            }
            ObjectTypeCode::Contract => {
                Contract::raw_decode(buf).map(|(obj, buf)| (StandardObject::Contract(obj), buf))
            }
            _ => {
                unreachable!();
            }
        }
    }
}
