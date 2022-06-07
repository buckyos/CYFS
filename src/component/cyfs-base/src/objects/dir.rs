use crate::codec as cyfs_base;
use crate::*;

use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct Attributes {
    flags: u32,
}

impl Attributes {
    pub fn new(flags: u32) -> Self {
        Self { flags }
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Self { flags: 0 }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum InnerNode {
    // 如果是一个子Dir，把该Dir编码放在Body，此处持有DirId
    // 如果是一个Diff，把Diff编码放在Body，此处持有DiffId
    // 如果是一个大File，把File编码放在Body，此处持有FileId
    ObjId(ObjectId),

    // 以下针对单Chunk的File

    // 只有一个Chunk的File
    Chunk(ChunkId),

    // 只有一个Chunk的File的小文件，持有NDNObjectList::parent_chunk的buffer索引
    IndexInParentChunk(u32, u32),
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct InnerNodeInfo {
    attributes: Attributes,
    node: InnerNode,
}

impl InnerNodeInfo {
    pub fn new(attributes: Attributes, node: InnerNode) -> Self {
        Self { attributes, node }
    }
    pub fn get_object_id(&self) -> ObjectId {
        //需要计算
        unimplemented!();
    }

    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    pub fn node(&self) -> &InnerNode {
        &self.node
    }
}

pub type DirBodyDescObjectMap = std::collections::HashMap<String, InnerNodeInfo>;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct NDNObjectList {
    // Dir内部的多个小文件的File对象，打包成一个Chunk，放在Body
    // 此处持有该打包Chunk的一个Id
    pub parent_chunk: Option<ChunkId>,

    // 文件内部路径-文件信息字典
    pub object_map: DirBodyDescObjectMap,
}

impl NDNObjectList {
    pub fn new(parent_chunk: Option<ChunkId>) -> Self {
        Self {
            parent_chunk,
            object_map: HashMap::new(),
        }
    }

    pub fn parent_chunk(&self) -> Option<&ChunkId> {
        self.parent_chunk.as_ref()
    }

    pub fn object_map(&self) -> &HashMap<String, InnerNodeInfo> {
        &self.object_map
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum NDNObjectInfo {
    // 要么把NDNObjectList编码成一个Chunk，并在Body内持有
    // 此处持有ChunkId
    Chunk(ChunkId),

    // 要么直接内置对象列表
    ObjList(NDNObjectList),
}

#[derive(Clone, Debug, RawDecode)]
pub struct DirDescContent {
    attributes: Attributes,
    obj_list: NDNObjectInfo,
}

impl RawEncode for DirDescContent {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ret = if purpose == &Some(RawEncodePurpose::Hash) {
            self.attributes.raw_measure(purpose)? + ChunkId::raw_bytes().unwrap()
        } else {
            self.attributes.raw_measure(purpose)? + self.obj_list.raw_measure(purpose)?
        };
        Ok(ret)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let ret = if purpose == &Some(RawEncodePurpose::Hash) {
            let remain_buf = self.attributes.raw_encode(buf, purpose)?;
            match &self.obj_list {
                NDNObjectInfo::Chunk(chunk_id) => chunk_id.raw_encode(remain_buf, purpose).unwrap(),
                NDNObjectInfo::ObjList(list) => {
                    // 把list编码到chunk
                    let size = list.raw_measure(&Some(RawEncodePurpose::Serialize))?;
                    let mut chunk_buf = vec![0u8; size];
                    let left_buf =
                        list.raw_encode(&mut chunk_buf, &Some(RawEncodePurpose::Serialize))?;
                    assert!(left_buf.len() == 0);

                    let chunk_id = ChunkId::calculate_sync(&chunk_buf).unwrap();
                    assert!(chunk_id.len() == size);

                    chunk_id.raw_encode(remain_buf, purpose).unwrap()
                }
            }
        } else {
            let buf = self.attributes.raw_encode(buf, purpose)?;
            self.obj_list.raw_encode(buf, purpose)?
        };
        Ok(ret)
    }
}

impl DirDescContent {
    pub fn new(attributes: Attributes, obj_list: NDNObjectInfo) -> Self {
        Self {
            attributes,
            obj_list,
        }
    }

    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    pub fn obj_list(&self) -> &NDNObjectInfo {
        &self.obj_list
    }
}

impl DescContent for DirDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Dir.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

pub type DirBodyContentObjectList = HashMap<ObjectId, Vec<u8>>;

#[derive(Clone, Debug)]
pub enum DirBodyContent {
    // 要么把ObjList压缩放chunk
    // TODO? 但是Chunk要放在哪里？
    Chunk(ChunkId),

    // 要么直接展开持有
    ObjList(DirBodyContentObjectList),
}

impl BodyContent for DirBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl TryFrom<protos::DirBodyContent> for DirBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::DirBodyContent) -> BuckyResult<Self> {
        let ret = match value.get_field_type() {
            protos::DirBodyContent_Type::Chunk => {
                Self::Chunk(ProtobufCodecHelper::decode_buf(value.take_chunk_id())?)
            }
            protos::DirBodyContent_Type::ObjList => {
                let mut ret = HashMap::new();
                for mut item in value.take_obj_list().into_iter() {
                    let (k, _) = ObjectId::raw_decode(&item.get_obj_id())?;
                    let v = item.take_value();
                    if let Some(_old) = ret.insert(k, v) {
                        error!("decode dir obj_list from protobuf got repeated key! {}", k);
                    }
                }
                Self::ObjList(ret)
            }
        };

        Ok(ret)
    }
}

impl TryFrom<&DirBodyContent> for protos::DirBodyContent {
    type Error = BuckyError;

    fn try_from(value: &DirBodyContent) -> BuckyResult<Self> {
        let mut ret = protos::DirBodyContent::new();
        match value {
            DirBodyContent::Chunk(id) => {
                ret.set_field_type(protos::DirBodyContent_Type::Chunk);
                ret.set_chunk_id(id.to_vec()?);
            }
            DirBodyContent::ObjList(list) => {
                ret.set_field_type(protos::DirBodyContent_Type::ObjList);
                let mut item_list = Vec::new();
                for (k, v) in list {
                    let mut item = protos::DirBodyContent_ObjItem::new();
                    item.set_obj_id(k.to_vec()?);
                    item.set_value(v.to_owned());
                    item_list.push(item);
                }
                ret.set_obj_list(item_list.into());
            }
        }

        Ok(ret)
    }
}

inner_impl_default_protobuf_raw_codec!(DirBodyContent);

pub type DirType = NamedObjType<DirDescContent, DirBodyContent>;
pub type DirBuilder = NamedObjectBuilder<DirDescContent, DirBodyContent>;

pub type DirDesc = NamedObjectDesc<DirDescContent>;
pub type DirId = NamedObjectId<DirType>;
pub type Dir = NamedObjectBase<DirType>;

impl DirDesc {
    pub fn dir_id(&self) -> DirId {
        DirId::try_from(self.calculate_id()).unwrap()
    }
}

impl Dir {
    pub fn new(
        dir_attributes: Attributes,
        obj_desc: NDNObjectInfo,
        obj_map: HashMap<ObjectId, Vec<u8>>,
    ) -> DirBuilder {
        let desc_content = DirDescContent::new(dir_attributes, obj_desc);
        let body_content = DirBodyContent::ObjList(obj_map);
        DirBuilder::new(desc_content, body_content)
    }

    pub fn get_data_from_body(&self, id: &ObjectId) -> Option<&Vec<u8>> {
        match self.body() {
            Some(body) => {
                match body.content() {
                    DirBodyContent::ObjList(list) => list.get(id),
                    DirBodyContent::Chunk(_chunk_id) => {
                        // 不处理这种情况，上层需要对这种情况进一步展开成ObjList模式才可以进一步查询
                        None
                    }
                }
            }
            None => None,
        }
    }

    // desc content最大支持65535长度，如果超出此长度，需要切换为chunk模式
    pub fn check_and_fix_desc_limit(&mut self) -> BuckyResult<()> {
        let size = self.desc().content().raw_measure(&None)?;
        if size > u16::MAX as usize {
            match &self.desc_mut().content().obj_list {
                NDNObjectInfo::ObjList(list) => {
                    let chunk = list.to_vec()?;
                    let chunk_id = ChunkId::calculate_sync(&chunk)?;
                    drop(list);

                    match self.body_mut() {
                        Some(body) => match body.content_mut() {
                            DirBodyContent::ObjList(list) => {
                                info!("will convert dir desc content list to chunk: list len={}, chunk_id={}",
                                    size, chunk_id);
                                list.insert(chunk_id.object_id(), chunk);
                                drop(list);

                                self.desc_mut().content_mut().obj_list =
                                    NDNObjectInfo::Chunk(chunk_id.clone());
                            }
                            DirBodyContent::Chunk(_chunk_id) => {
                                // 不支持body的chunk模式
                                let msg = format!(
                                    "fix dir desc limit not support body chunk mode! dir={}",
                                    self.desc().dir_id()
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
                            }
                        },
                        None => {
                            info!("will convert dir desc content list to chunk: list len={}, chunk_id={}",
                                    size, chunk_id);

                            // 如果dir不存在body，那么要动态的创建body
                            let mut object_list = DirBodyContentObjectList::new();
                            object_list.insert(chunk_id.object_id(), chunk);

                            let builder =
                                ObjectMutBodyBuilder::new(DirBodyContent::ObjList(object_list));
                            let body = builder.update_time(bucky_time_now()).build();

                            *self.body_mut() = Some(body);

                            self.desc_mut().content_mut().obj_list =
                                NDNObjectInfo::Chunk(chunk_id.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use std::collections::HashMap;

    #[test]
    fn dir() {
        let inner_node =
            InnerNodeInfo::new(Attributes::default(), InnerNode::ObjId(ObjectId::default()));

        let mut object_map = HashMap::new();
        object_map.insert("path1".to_owned(), inner_node);
        let list = NDNObjectList {
            parent_chunk: None,
            object_map,
        };
        // 第一种情况，构造一个普通大小的dir，内容可以放到desc里面
        let attr = Attributes::new(0xFFFF);
        let builder = Dir::new(
            attr.clone(),
            NDNObjectInfo::ObjList(list.clone()),
            HashMap::new(),
        );
        let dir = builder.no_create_time().update_time(0).build();
        let dir_id = dir.desc().calculate_id();
        println!("dir id={}", dir_id);
        assert_eq!(
            dir_id.to_string(),
            "7jMmeXZpjj4YRfshnxsTqyDbqyo9zDoDA5phG9AXDC7X"
        );
        let buf = dir.to_vec().unwrap();
        let hash = hash_data(&buf);
        info!("dir hash={}", hash);
        // 第二种情况，对于超大内容的dir，使用chunk模式，但和上面一种模式是对等的
        let data = list.to_vec().unwrap();
        let chunk_id = ChunkId::calculate_sync(&data).unwrap();

        // chunk可以放到body缓存里面，方便查找；也可以独立存放，但dir在解析时候需要再次查找该chunk可能会耗时久，以及查找失败等情况
        let mut obj_map = HashMap::new();
        obj_map.insert(chunk_id.object_id(), data);

        let builder = Dir::new(attr.clone(), NDNObjectInfo::Chunk(chunk_id), obj_map);
        let dir = builder.no_create_time().update_time(0).build();
        let dir_id2 = dir.desc().calculate_id();
        info!("dir id2={}", dir_id2);
        let buf = dir.to_vec().unwrap();
        let hash = hash_data(&buf);
        info!("dir2 hash={}", hash);

        let _dir3 = AnyNamedObject::clone_from_slice(&buf).unwrap();

        // 上述两种模式生成的dir_id应该是相同
        assert_eq!(dir_id, dir_id2);
    }

    #[test]
    fn test_fix_limit() {
        let inner_node =
            InnerNodeInfo::new(Attributes::default(), InnerNode::ObjId(ObjectId::default()));

        let mut object_map = HashMap::new();
        for i in 0..1024 * 10 {
            let path = format!("test dir path {}", i);
            object_map.insert(path, inner_node.clone());
        }
        let list = NDNObjectList {
            parent_chunk: None,
            object_map,
        };

        let builder = Dir::new(
            Attributes::default(),
            NDNObjectInfo::ObjList(list),
            HashMap::new(),
        );
        let mut dir = builder.no_create_time().update_time(0).build();
        *dir.body_mut() = None;
        let ret = dir.raw_measure(&None).unwrap_err();
        assert!(ret.code() == BuckyErrorCode::OutOfLimit);

        dir.check_and_fix_desc_limit().unwrap();

        let size = dir.raw_measure(&None).unwrap();
        println!("dir len: {}", size);
    }
}
