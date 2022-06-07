use crate::*;

use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub struct FileDescContent {
    len: u64,
    hash: HashValue,
}

impl FileDescContent {
    pub fn new(len: u64, hash: HashValue) -> Self {
        Self { len, hash }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn hash(&self) -> &HashValue {
        &self.hash
    }
}

impl DescContent for FileDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::File.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

impl RawEncode for FileDescContent {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let size =
            0 + self.len.raw_measure(purpose).map_err(|e| {
                log::error!("FileDescContent::raw_measure/len error:{}", e);
                e
            })? + self.hash.raw_measure(purpose).map_err(|e| {
                log::error!("FileDescContent::raw_measure/hash error:{}", e);
                e
            })?;
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for FileDescContent",
            ));
        }

        let buf = self.len.raw_encode(buf, purpose).map_err(|e| {
            log::error!("FileDescContent::raw_encode/len error:{}", e);
            e
        })?;

        let buf = self.hash.raw_encode(buf, purpose).map_err(|e| {
            log::error!("FileDescContent::raw_encode/hash error:{}", e);
            e
        })?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for FileDescContent {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (len, buf) = u64::raw_decode(buf).map_err(|e| {
            log::error!("FileDescContent::raw_decode/len error:{}", e);
            e
        })?;

        let (hash, buf) = HashValue::raw_decode(buf).map_err(|e| {
            log::error!("FileDescContent::raw_decode/hash error:{}", e);
            e
        })?;

        Ok((Self { len, hash }, buf))
    }
}

#[derive(Debug, Clone)]
pub enum ChunkBundleHashMethod {
    // calc hash in list seq order
    Serial = 0,
}

impl ChunkBundleHashMethod {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Serial => "serial",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChunkBundle{
    list: Vec<ChunkId>,
    hash_method: ChunkBundleHashMethod,
}

impl ChunkBundle {
    pub fn new(list: Vec<ChunkId>, hash_method: ChunkBundleHashMethod) -> Self {
        Self {
            list,
            hash_method,
        }
    }

    pub fn len(&self) -> u64 {
        self.list.iter().fold(0u64, |acc, id| {
            acc + id.len() as u64
        })
    }

    pub fn hash_method(&self) -> &ChunkBundleHashMethod {
        &self.hash_method
    }

    pub fn chunk_list(&self) -> &Vec<ChunkId> {
        &self.list
    }

    pub fn calc_hash_value(&self) -> HashValue {
        match self.hash_method {
            ChunkBundleHashMethod::Serial => {
                self.calc_serial_hash_value()
            }
        }
    }

    fn calc_serial_hash_value(&self) -> HashValue {
        use sha2::Digest;

        let mut sha256 = sha2::Sha256::new();
        self.list.iter().for_each(|id| {
            sha256.input(id.as_slice());
        });

        sha256.result().into()
    }
}

#[derive(Debug, Clone)]
pub enum ChunkList {
    ChunkInList(Vec<ChunkId>),
    ChunkInFile(FileId),
    ChunkInBundle(ChunkBundle),
}

impl ChunkList {
    pub fn inner_chunk_list(&self) -> Option<&Vec<ChunkId>> {
        match self {
            Self::ChunkInList(list) => Some(list),
            Self::ChunkInBundle(bundle) => {
                Some(bundle.chunk_list())
            }
            Self::ChunkInFile(_) => None,
        }
    }
}

/*
impl RawEncode for ChunkList {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let len = match self {
            ChunkList::ChunkInList(list) => {
                1 + list.raw_measure(purpose).map_err(|e| {
                    log::error!("ChunkList::raw_measure/list error:{}", e);
                    e
                })?
            }
            ChunkList::ChunkInFile(id) => {
                1 + id.raw_measure(purpose).map_err(|e| {
                    log::error!("ChunkList::raw_measure/id error:{}", e);
                    e
                })?
            }
            ChunkList::ChunkInBundle(bundle) => {
                1 + bundle.raw_measure(purpose).map_err(|e| {
                    log::error!("ChunkBundle::raw_measure/id error:{}", e);
                    e
                })?
            }
        };
        Ok(len)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for ChunkList",
            ));
        }
        let mut buf = buf;
        match self {
            ChunkList::ChunkInList(list) => {
                buf = 0u8.raw_encode(buf, purpose).map_err(|e| {
                    log::error!("ChunkList::raw_encode/flag_0 error:{}", e);
                    e
                })?;

                buf = list.raw_encode(buf, purpose).map_err(|e| {
                    log::error!("ChunkList::raw_encode/list error:{}", e);
                    e
                })?;
            }
            ChunkList::ChunkInFile(id) => {
                buf = 1u8.raw_encode(buf, purpose).map_err(|e| {
                    log::error!("ChunkList::raw_encode/flag_1 error:{}", e);
                    e
                })?;

                buf = id.raw_encode(buf, purpose).map_err(|e| {
                    log::error!("ChunkList::raw_encode/id error:{}", e);
                    e
                })?;
            }
            ChunkList::ChunkInBundle(bundle) => {
                buf = 2u8.raw_encode(buf, purpose).map_err(|e| {
                    log::error!("ChunkList::raw_encode/flag_0 error:{}", e);
                    e
                })?;

                buf = bundle.raw_encode(buf, purpose).map_err(|e| {
                    log::error!("ChunkList::raw_encode/list error:{}", e);
                    e
                })?;
            }
        }

        Ok(buf)
    }
}

impl RawDecode<'_> for ChunkList {
    fn raw_decode(buf: &[u8]) -> BuckyResult<(Self, &[u8])> {
        let (list_type, buf) = u8::raw_decode(buf).map_err(|e| {
            log::error!("ChunkList::raw_decode/id error:{}", e);
            e
        })?;

        match list_type {
            0 => {
                let (list, buf) = Vec::<ChunkId>::raw_decode(buf).map_err(|e| {
                    log::error!("ChunkList::raw_decode/list error:{}", e);
                    e
                })?;

                Ok((ChunkList::ChunkInList(list), buf))
            }
            1 => {
                let (id, buf) = FileId::raw_decode(buf).map_err(|e| {
                    log::error!("ChunkList::raw_decode/id error:{}", e);
                    e
                })?;

                Ok((ChunkList::ChunkInFile(id), buf))
            }
            2 => {
                let (bundle, buf) = ChunkBundle::raw_decode(buf).map_err(|e| {
                    log::error!("ChunkBundle::raw_decode error:{}", e);
                    e
                })?;

                Ok((ChunkList::ChunkInBundle(bundle), buf))
            }
            _ => {
                unreachable!()
            }
        }
    }
}
*/
/*
impl<'v> RawDiffWithContext<'v, VecDiffContext<'v, ChunkId>> for ChunkList {
    fn diff_measure(
        &self,
        right: &'v Self,
        ctx: &mut VecDiffContext<'v, ChunkId>,
    ) -> BuckyResult<usize> {
        let size = u8::raw_bytes().unwrap()
            + match self {
                ChunkList::ChunkInList(left_list) => match right {
                    ChunkList::ChunkInList(right_list) => left_list.diff_measure(right_list, ctx),
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::NotMatch,
                        "chunk list type not match, left is chunk in list, right is chunk in file",
                    )),
                },
                ChunkList::ChunkInFile(left_file_id) => match right {
                    ChunkList::ChunkInFile(right_file_id) => {
                        left_file_id.diff_measure(right_file_id)
                    }
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::NotMatch,
                        "chunk list type not match, left is chunk in file, right is chunk in list",
                    )),
                },
            }?;
        Ok(size)
    }

    fn diff<'d>(
        &self,
        right: &Self,
        buf: &'d mut [u8],
        ctx: &mut VecDiffContext<'v, ChunkId>,
    ) -> BuckyResult<&'d mut [u8]> {
        let size = self.raw_measure(&None).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_diff] not enough buffer for VecDiffContext",
            ));
        }

        let buf = match self {
            ChunkList::ChunkInList(left_list) => match right {
                ChunkList::ChunkInList(right_list) => {
                    let buf = 0u8.raw_encode(buf, &None).map_err(|e| {
                        log::error!("ChunkList::diff/flag_0 error:{}", e);
                        e
                    })?;

                    left_list.diff(right_list, buf, ctx)
                }
                _ => Err(BuckyError::new(
                    BuckyErrorCode::NotMatch,
                    "chunk list type not match, left is chunk in list, right is chunk in file",
                )),
            },
            ChunkList::ChunkInFile(left_file_id) => match right {
                ChunkList::ChunkInFile(right_file_id) => {
                    let buf = 1u8
                        .raw_encode(buf, &None)
                        .map_err(|e| {
                            log::error!("ChunkList::diff/flag_1 error:{}", e);
                            e
                        })
                        .map_err(|e| {
                            log::error!("ChunkList::patch/left_file_id error:{}", e);
                            e
                        })?;

                    left_file_id.diff(right_file_id, buf)
                }
                _ => Err(BuckyError::new(
                    BuckyErrorCode::NotMatch,
                    "chunk list type not match, left is chunk in file, right is chunk in list",
                )),
            },
        }?;

        Ok(buf)
    }
}

impl<'de> RawPatch<'de> for ChunkList {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (flag, buf) = u8::raw_decode(buf).map_err(|e| {
            log::error!("ChunkList::patch/flag error:{}", e);
            e
        })?;

        match flag {
            0u8=>{
                match self {
                    ChunkList::ChunkInList(left_list)=>{
                        let (left_list ,buf)= left_list.patch(buf).map_err(|e|{
                            log::error!("ChunkList::patch/left_list error:{}", e); 
                            e
                        })?;

                        Ok((ChunkList::ChunkInList(left_list),buf))
                    },
                    ChunkList::ChunkInFile(_left_file_id)=>{
                        Err(BuckyError::new(BuckyErrorCode::NotMatch,"decoded chunk list type not match, left is chunk in file, right is chunk in list"))
                    }
                }
            },
            1u8=>{
                match self {
                    ChunkList::ChunkInList(_left_list)=>{
                        Err(BuckyError::new(BuckyErrorCode::NotMatch,"decoded chunk list type not match, left is chunk in list, right is chunk in file"))
                    },
                    ChunkList::ChunkInFile(left_file_id)=>{
                        let (left_file_id, buf) = left_file_id.patch(buf).map_err(|e|{
                            log::error!("ChunkList::patch/left_file_id error:{}", e); 
                            e
                        })?;
                        Ok((ChunkList::ChunkInFile(left_file_id), buf))
                    }
                }
            },
            _=>{
                Err(BuckyError::new(BuckyErrorCode::NotSupport, "decoded chunk list type is not support"))
            }
        }
    }
}
*/

#[derive(Clone, Debug)]
pub struct FileBodyContent {
    chunk_list: ChunkList,
}

impl BodyContent for FileBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl FileBodyContent {
    pub fn new(chunk_list: ChunkList) -> Self {
        Self { chunk_list }
    }

    pub fn chunk_list(&self) -> &ChunkList {
        &self.chunk_list
    }

    pub fn inner_chunk_list(&self) -> Option<&Vec<ChunkId>> {
        self.chunk_list.inner_chunk_list()
    }

    pub fn into_chunk_list(self) -> ChunkList {
        self.chunk_list
    }
}

/*
impl<'v> RawDiffWithContext<'v, VecDiffContext<'v, ChunkId>> for FileBodyContent {
    fn diff_measure(
        &self,
        right: &'v Self,
        ctx: &mut VecDiffContext<'v, ChunkId>,
    ) -> BuckyResult<usize> {
        self.chunk_list.diff_measure(&right.chunk_list, ctx)
    }

    fn diff<'d>(
        &self,
        right: &Self,
        buf: &'d mut [u8],
        ctx: &mut VecDiffContext<'v, ChunkId>,
    ) -> BuckyResult<&'d mut [u8]> {
        self.chunk_list.diff(&right.chunk_list, buf, ctx)
    }
}

impl<'de> RawPatch<'de> for FileBodyContent {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (chunk_list, buf) = self.chunk_list.patch(buf).map_err(|e| {
            log::error!("FileBodyContent::patch/chunk_list error:{}", e);
            e
        })?;
        Ok((Self { chunk_list }, buf))
    }
}
*/

// 使用protobuf对body进行编解码
impl TryFrom<protos::ChunkList> for ChunkList {
    type Error = BuckyError;

    fn try_from(mut value: protos::ChunkList) -> BuckyResult<Self> {
        let ret = match value.get_field_type() {
            protos::ChunkList_Type::ChunkInFile => {
                Self::ChunkInFile(ProtobufCodecHelper::decode_buf(value.take_file_id())?)
            }
            protos::ChunkList_Type::ChunkInList => Self::ChunkInList(
                ProtobufCodecHelper::decode_buf_list(value.take_chunk_id_list())?,
            ),
            protos::ChunkList_Type::ChunkInBundle => {
                let list: Vec<ChunkId> = ProtobufCodecHelper::decode_buf_list(value.take_chunk_id_list())?;
                match value.get_hash_method() {
                    protos::ChunkList_HashMethod::Serial => {
                        let bundle = ChunkBundle::new(list, ChunkBundleHashMethod::Serial);
                        Self::ChunkInBundle(bundle)
                    }
                }
            }
        };
        Ok(ret)
    }
}

impl TryFrom<&ChunkList> for protos::ChunkList {
    type Error = BuckyError;

    fn try_from(value: &ChunkList) -> BuckyResult<Self> {
        let mut ret = protos::ChunkList::new();

        match value {
            ChunkList::ChunkInFile(id) => {
                ret.set_field_type(protos::ChunkList_Type::ChunkInFile);
                ret.set_file_id(id.to_vec()?);
            }
            ChunkList::ChunkInList(list) => {
                ret.set_field_type(protos::ChunkList_Type::ChunkInList);
                ret.set_chunk_id_list(ProtobufCodecHelper::encode_buf_list(list)?);
            }
            ChunkList::ChunkInBundle(bundle) => {
                ret.set_field_type(protos::ChunkList_Type::ChunkInBundle);
                ret.set_chunk_id_list(ProtobufCodecHelper::encode_buf_list(bundle.chunk_list())?);
                match bundle.hash_method() {
                    ChunkBundleHashMethod::Serial => {
                        ret.set_hash_method(protos::ChunkList_HashMethod::Serial);
                    }
                }
            }
        }

        Ok(ret)
    }
}

impl TryFrom<protos::FileBodyContent> for FileBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::FileBodyContent) -> BuckyResult<Self> {
        Ok(Self {
            chunk_list: ChunkList::try_from(value.take_chunk_list())?,
        })
    }
}

impl TryFrom<&FileBodyContent> for protos::FileBodyContent {
    type Error = BuckyError;

    fn try_from(value: &FileBodyContent) -> BuckyResult<Self> {
        let mut ret = protos::FileBodyContent::new();

        ret.set_chunk_list(protos::ChunkList::try_from(&value.chunk_list)?);
        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(FileBodyContent);

pub type FileType = NamedObjType<FileDescContent, FileBodyContent>;
pub type FileBuilder = NamedObjectBuilder<FileDescContent, FileBodyContent>;

pub type FileDesc = NamedObjectDesc<FileDescContent>;
pub type FileId = NamedObjectId<FileType>;
pub type File = NamedObjectBase<FileType>;

impl FileDesc {
    pub fn file_id(&self) -> FileId {
        FileId::try_from(self.calculate_id()).unwrap()
    }
}

impl File {
    pub fn new(owner: ObjectId, len: u64, hash: HashValue, chunk_list: ChunkList) -> FileBuilder {
        let desc_content = FileDescContent::new(len, hash);
        let body_content = FileBodyContent::new(chunk_list);

        FileBuilder::new(desc_content, body_content).owner(owner)
    }

    pub fn new_no_owner(len: u64, hash: HashValue, chunk_list: ChunkList) -> FileBuilder {
        let desc_content = FileDescContent::new(len, hash);
        let body_content = FileBodyContent::new(chunk_list);

        FileBuilder::new(desc_content, body_content)
    }

    pub fn len(&self) -> u64 {
        self.desc().content().len()
    }

    pub fn hash(&self) -> &HashValue {
        self.desc().content().hash()
    }
}

#[cfg(test)]
mod test_file {
    use crate::*;
    use std::str::FromStr;
    //use std::convert::From;
    //use std::path::Path;

    // #[test]
    // fn file_load() {
    //     let p = Path::new("f:\\temp\\file.obj");
    //     if p.exists() {
    //         let mut v = Vec::<u8>::new();
    //         let (file, _) = File::decode_from_file(p, &mut v).unwrap();
    //         println!("{:?}", file);
    //     }
    //
    // }

    #[test]
    fn file() {
        let owner = ObjectId::default();
        let hash = HashValue::default();

        let chunk_list = vec![ChunkId::default(), ChunkId::default()];

        let chunk_list = ChunkList::ChunkInList(chunk_list);
        let chunk_list_2 = chunk_list.clone();
        let _chunk_list_3 = chunk_list.clone();

        let file = File::new(owner, 100, hash, chunk_list)
            .no_create_time()
            .build();

        let file_id = file.desc().file_id();
        let _chunk_list_4 = ChunkList::ChunkInFile(file_id.clone());

        let hash = hash.clone();

        let file2 = File::new(owner, 100, hash, chunk_list_2)
            .no_create_time()
            .build();
        let file_id_2 = file2.desc().file_id();

        println!("\n file_id:{:?}, \n file_id_2:{:?}\n\n", file_id, file_id_2);

        // assert!(false);
        assert!(file_id.to_string() == file_id_2.to_string());

        // let p = Path::new("f:\\temp\\file.obj");
        // if p.parent().unwrap().exists() {
        //     file.encode_to_file(p, false);
        // }
        // let p = Path::new("f:\\temp\\chunk_256.obj");
        // if p.parent().unwrap().exists() {
        //     chunk_list_3.encode_to_file(p, false);
        // }
        // let p = Path::new("f:\\temp\\chunk_in_file.obj");
        // if p.parent().unwrap().exists() {
        //     chunk_list_4.encode_to_file(p, false);
        // }
    }

    #[test]
    fn test_codec() {
        let chunk_list = ["7C8WXUGc4cDjrFrcaYE28FwkykTc7D67sq6nVpwmc73C","7C8WXUGvbdJrK7X7wfPzhWFKU1c14GQiMcvrmqMwmQm3","7C8WXUGqpDS3fwmQhX4PT8zJKBcPPT7SWznKCzSJyX4h","7C8WXUH97ftxKchxX1xPYfjUqk4TVgARnRSA1Z6vki2u","7C8WXUGnjjTWstqyEdAMtsUPHESV8AM4JUzNABnKiCmD","7C8xA3NNwQPw5UFBQa4PL3u1iN9htZj2se5j54FGE9cv"];
        let list: Vec<ChunkId> = chunk_list.iter().map(|id| ChunkId::from_str(&id).unwrap()).collect();

        let chunks  = ChunkList::ChunkInList(list);
        let body = FileBodyContent::new(chunks);
        let buf = body.to_vec().unwrap();
        println!("body len={}", buf.len());
    
        let code = "00080a02002f3d25175662554800000000434c42a2c2a288367e7533c9aec22813d876bbe84ab6cf370f0a130000002800000000016a8000c7b07e826cb5f241178c5d22e4b5dc743c55e1443e8b4405fc5f04c0ac5a44d800002f3d28a5280da6000140d10ace01080012205c00004000140c3c275d3399655537e1482621cd95d7a4e143e3abc5d65bfc0f12205c000040009d2f79890697bcc7d47d1a571f5bcbce2c45a8be8fb630db2c31f612205c0000400079cbf395974b7512b0ef24f974076c606041002ef8fbe60ba1763e12205c00004000f9ce26d7332885a7a381ed87c4a994d823491f9701c89a54bda35a12205c0000400063073223a62acb1a53e45d2d363b5aac5355f4649e0da8355a833812205c00802a0095289410d3e504516a1afd429021d4767cb3be26a0ff288716fabb";
        let mut buf = vec![];
        let file = File::clone_from_hex(&code, &mut buf).unwrap();
        println!("{}", file.format_json());

        //let new_code = file.to_vec().unwrap();
        //let new_code = hex::encode(&new_code);
        //assert_eq!(new_code, code);
    }
}
