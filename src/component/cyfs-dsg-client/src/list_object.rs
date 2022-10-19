use std::marker::PhantomData;
use cyfs_base::*;

pub trait ObjTypeCode: RawEncode + for <'de> RawDecode<'de> + Clone + Sync + Send {
    fn obj_type() -> u16;
}

#[macro_export]
macro_rules! define_obj_type {
    ( $name: ident, $value: expr) => {
        #[derive(Clone)]
        pub struct $name;
        impl ObjTypeCode for $name {
            fn obj_type() -> u16 {
                $value
            }
        }

        impl cyfs_base::RawEncode for $name {
            fn raw_measure(&self, _purpose: &Option<cyfs_base::RawEncodePurpose>) -> cyfs_base::BuckyResult<usize> {
                unreachable!()
            }

            fn raw_encode<'a>(
                &self,
                _buf: &'a mut [u8],
                _purpose: &Option<cyfs_base::RawEncodePurpose>,
            ) -> cyfs_base::BuckyResult<&'a mut [u8]> {
                unreachable!()
            }
        }

        impl <'de> cyfs_base::RawDecode<'de> for $name {
            fn raw_decode(_buf: &'de [u8]) -> cyfs_base::BuckyResult<(Self, &'de [u8])> {
                unreachable!()
            }
        }
    };
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct ListDescContent<OT: ObjTypeCode + RawEncode + for <'a> RawDecode<'a> + Clone + Send + Sync> {
    list_hash: HashValue,
    #[cyfs(skip)]
    obj_type: PhantomData<OT>,
}

impl <OT: ObjTypeCode> DescContent for ListDescContent<OT> {
    fn obj_type() -> u16 {
        OT::obj_type()
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct ListBodyContent<T: RawEncode + for <'a> RawDecode<'a> + Clone + Send + Sync> {
    list: Vec<T>,
}

impl <T: RawEncode + for <'a> RawDecode<'a> + Clone + Send + Sync> BodyContent for ListBodyContent<T> {

}

pub type ListType<OT, T> = NamedObjType<ListDescContent<OT>, ListBodyContent<T>>;
pub type ListBuilder<OT, T> = NamedObjectBuilder<ListDescContent<OT>, ListBodyContent<T>>;
pub type ListObject<OT, T> = NamedObjectBase<ListType<OT, T>>;

pub trait TListObject<T: RawEncode + for <'a> RawDecode<'a> + Clone + Send + Sync>: Sized {
    fn new(list: Vec<T>) -> Self;
    fn list_hash(&self) -> &HashValue;
    fn into_list(self) -> Vec<T>;
    fn list(&self) -> &Vec<T>;
}

impl <OT: ObjTypeCode, T: RawEncode + for <'a> RawDecode<'a> + Clone + Send + Sync> TListObject<T> for ListObject<OT, T> {
    fn new(list: Vec<T>) -> Self {
        let list_hash = hash_data(list.to_vec().unwrap().as_slice());
        let body = ListBodyContent {
            list
        };
        let desc = ListDescContent {
            list_hash,
            obj_type: Default::default()
        };

        ListBuilder::new(desc, body).no_create_time().build()
    }

    fn list_hash(&self) -> &HashValue {
        &self.desc().content().list_hash
    }

    fn into_list(self) -> Vec<T> {
        self.into_body().unwrap().into_content().list
    }

    fn list(&self) -> &Vec<T> {
        &self.body().as_ref().unwrap().content().list
    }
}
