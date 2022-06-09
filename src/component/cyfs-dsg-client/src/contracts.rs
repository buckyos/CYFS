use crate::{obj_id, protos};
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use std::{convert::TryFrom, fmt::Debug, str::FromStr};

pub fn dsg_dec_id() -> ObjectId {
    DecApp::generate_id(
        ObjectId::from_str("5r4MYfFPKMeHa1fec7dHKmBfowySBfVFvRQvKB956dnF").unwrap(),
        "cyfs dsg service",
    )
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgNonWitness {}

impl Debug for DsgNonWitness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NonWitness")
    }
}

#[derive(Clone)]
pub struct DsgIgnoreWitness {
    buffer: Vec<u8>,
}

impl Debug for DsgIgnoreWitness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IgnoreWitness")
    }
}

impl RawEncode for DsgIgnoreWitness {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(self.buffer.len())
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < self.buffer.len() {
            Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "no enough buffer",
            ))
        } else {
            buf[..self.buffer.len()].copy_from_slice(self.buffer.as_slice());
            Ok(&mut buf[self.buffer.len()..])
        }
    }
}

impl<'de> RawDecode<'de> for DsgIgnoreWitness {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let o = Self {
            buffer: Vec::from(buf),
        };
        Ok((o, &buf[buf.len()..]))
    }
}

/**
 * 数据源模式
 * 不可变 指定chunk list
 * 可变 指定空间
 */
#[derive(Clone, Debug)]
pub enum DsgDataSource {
    Immutable(Vec<ChunkId> /*chunk list*/),
    Mutable(u64 /*total space*/),
}

/**
 * 还应当包含
 * 带宽流量约束
 * 计费相关字段
*/
#[derive(Clone, Debug)]
pub struct DsgCacheStorage {
    // http 可访问性
    pub pub_http: Option<String /*url*/>,
    // cyfs 可访问性
    pub pub_cyfs: bool,
}

impl TryFrom<&DsgCacheStorage> for protos::CacheStorage {
    type Error = BuckyError;

    fn try_from(rust: &DsgCacheStorage) -> BuckyResult<Self> {
        let mut proto = protos::CacheStorage::new();
        if let Some(url) = &rust.pub_http {
            proto.set_pub_http(url.clone());
        }
        proto.set_pub_cyfs(rust.pub_cyfs);
        Ok(proto)
    }
}

impl TryFrom<protos::CacheStorage> for DsgCacheStorage {
    type Error = BuckyError;

    fn try_from(mut proto: protos::CacheStorage) -> BuckyResult<Self> {
        Ok(Self {
            pub_http: if proto.has_pub_http() {
                Some(proto.take_pub_http())
            } else {
                None
            },
            pub_cyfs: proto.get_pub_cyfs(),
        })
    }
}

/**
 * 还应当包含计费相关字段
*/
#[derive(Clone, Debug)]
pub struct DsgBackupStorage {
    _reserved: u32,
}

impl DsgBackupStorage {
    pub fn new() -> Self {
        Self { _reserved: 0 }
    }
}

impl TryFrom<&DsgBackupStorage> for protos::BackupStorage {
    type Error = BuckyError;

    fn try_from(_rust: &DsgBackupStorage) -> BuckyResult<Self> {
        let mut proto = protos::BackupStorage::new();
        proto.set_reserved(0);
        Ok(proto)
    }
}

impl TryFrom<protos::BackupStorage> for DsgBackupStorage {
    type Error = BuckyError;

    fn try_from(mut _proto: protos::BackupStorage) -> BuckyResult<Self> {
        Ok(Self { _reserved: 0 })
    }
}

/**
 * 介质类型
 * 缓存类型 提供可用性和公网访问性
 * 备份类型 提供可靠性，可恢复
 */
#[derive(Clone, Debug)]
pub enum DsgStorage {
    Cache(DsgCacheStorage),
    Backup(DsgBackupStorage),
}

#[derive(Clone, Debug)]
pub struct DsgContractDesc<T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    pub data_source: DsgDataSource,
    pub storage: DsgStorage,
    pub miner: ObjectId,
    pub start_at: u64,
    pub end_at: u64,
    pub witness_dec_id: Option<ObjectId>,
    pub witness: T,
}

impl<T> TryFrom<&DsgContractDesc<T>> for protos::ContractDesc
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    type Error = BuckyError;

    fn try_from(rust: &DsgContractDesc<T>) -> BuckyResult<Self> {
        let mut proto = protos::ContractDesc::new();

        match &rust.data_source {
            DsgDataSource::Immutable(chunks) => {
                proto.set_data_source_type(protos::ContractDesc_DataSourceType::Immutable);
                let mut immut = protos::ImmutableDataSource::new();
                immut.set_chunks(ProtobufCodecHelper::encode_buf_list(chunks)?);
                proto.set_immut_data_source(immut);
            }
            DsgDataSource::Mutable(space) => {
                proto.set_data_source_type(protos::ContractDesc_DataSourceType::Mutable);
                proto.set_mut_data_source(*space);
            }
        }

        match &rust.storage {
            DsgStorage::Cache(cache) => {
                proto.set_storage_type(protos::ContractDesc_StorageType::Cache);
                proto.set_cache_storage(protos::CacheStorage::try_from(cache)?);
            }
            DsgStorage::Backup(backup) => {
                proto.set_storage_type(protos::ContractDesc_StorageType::Backup);
                proto.set_backup_storage(protos::BackupStorage::try_from(backup)?);
            }
        }

        proto.set_miner(rust.miner.to_vec()?);
        proto.set_start_at(rust.start_at);
        proto.set_end_at(rust.end_at);
        proto.set_witness(rust.witness.to_vec()?);
        if let Some(witness_dec_id) = &rust.witness_dec_id {
            proto.set_witness_dec_id(witness_dec_id.to_vec()?);
        }

        Ok(proto)
    }
}

impl<'de, T> TryFrom<protos::ContractDesc> for DsgContractDesc<T>
where
    T: Send + Sync + for<'a> RawDecode<'a> + RawEncode + Clone + Debug,
{
    type Error = BuckyError;

    fn try_from(mut proto: protos::ContractDesc) -> BuckyResult<Self> {
        Ok(Self {
            data_source: match proto.data_source_type {
                protos::ContractDesc_DataSourceType::Immutable => {
                    DsgDataSource::Immutable(ProtobufCodecHelper::decode_buf_list(
                        proto.take_immut_data_source().take_chunks(),
                    )?)
                }
                protos::ContractDesc_DataSourceType::Mutable => {
                    DsgDataSource::Mutable(proto.get_mut_data_source())
                }
            },
            storage: match proto.storage_type {
                protos::ContractDesc_StorageType::Cache => {
                    DsgStorage::Cache(DsgCacheStorage::try_from(proto.take_cache_storage())?)
                }
                protos::ContractDesc_StorageType::Backup => {
                    DsgStorage::Backup(DsgBackupStorage::try_from(proto.take_backup_storage())?)
                }
            },
            miner: ProtobufCodecHelper::decode_buf(proto.take_miner())?,
            start_at: proto.get_start_at(),
            end_at: proto.get_end_at(),
            witness: ProtobufCodecHelper::decode_buf(proto.take_witness())?,
            witness_dec_id: if proto.has_witness_dec_id() {
                Some(ProtobufCodecHelper::decode_buf(
                    proto.take_witness_dec_id(),
                )?)
            } else {
                None
            },
        })
    }
}

impl<T> RawEncode for DsgContractDesc<T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        ProtobufCodecHelper::raw_measure::<DsgContractDesc<T>, protos::ContractDesc>(&self, purpose)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        // info!("desc content encode");
        ProtobufCodecHelper::raw_encode::<DsgContractDesc<T>, protos::ContractDesc>(
            self, buf, purpose,
        )
    }
}

impl<'de, T> RawDecode<'de> for DsgContractDesc<T>
where
    T: Send + Sync + for<'a> RawDecode<'a> + RawEncode + Clone + Debug,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        // info!("desc content decode");
        ProtobufCodecHelper::raw_decode::<DsgContractDesc<T>, protos::ContractDesc>(buf)
    }
}

impl<T> Default for DsgContractDesc<T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug + Default,
{
    fn default() -> Self {
        Self {
            miner: ObjectId::default(),
            data_source: DsgDataSource::Mutable(0),
            storage: DsgStorage::Backup(DsgBackupStorage { _reserved: 0 }),
            start_at: 0,
            end_at: 0,
            witness_dec_id: None,
            witness: T::default(),
        }
    }
}

impl<T> DescContent for DsgContractDesc<T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn obj_type() -> u16 {
        obj_id::CONTRACT_OBJECT_TYPE
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgContractBody {}

impl BodyContent for DsgContractBody {}

pub type DsgContractObjectType<T> = NamedObjType<DsgContractDesc<T>, DsgContractBody>;
pub type DsgContractObject<T> = NamedObjectBase<DsgContractObjectType<T>>;

#[derive(Copy, Clone)]
pub struct DsgContractObjectRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    obj: &'a DsgContractObject<T>,
}

impl<'a, T> std::fmt::Display for DsgContractObjectRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DsgContractObject{{id={}, desc={:?}}}",
            self.id(),
            self.as_ref().desc().content()
        )
    }
}

impl<'a, T> AsRef<DsgContractObject<T>> for DsgContractObjectRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn as_ref(&self) -> &'a DsgContractObject<T> {
        self.obj
    }
}

impl<'a, T> From<&'a DsgContractObject<T>> for DsgContractObjectRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn from(obj: &'a DsgContractObject<T>) -> Self {
        Self { obj }
    }
}

impl<'a, T1, T2> Into<DsgContractObject<T1>> for DsgContractObjectRef<'a, T2>
where
    T1: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
    T2: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn into(self) -> DsgContractObject<T1> {
        DsgContractObject::<T1>::clone_from_slice(self.as_ref().to_vec().unwrap().as_slice())
            .unwrap()
    }
}

impl<'a, T> DsgContractObjectRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    pub fn create(
        stack: &SharedCyfsStack,
        desc: DsgContractDesc<T>,
    ) -> BuckyResult<DsgContractObject<T>> {
        let builder = NamedObjectBuilder::new(desc, DsgContractBody {});
        let contract = builder
            .dec_id(dsg_dec_id())
            .owner(stack.local_device_id().object_id().clone())
            .no_create_time()
            .build();
        Ok(contract)
    }

    pub fn consumer_signature(&self) -> Option<&Signature> {
        self.as_ref().signs().desc_signs().and_then(|signs| {
            signs.iter().find(|sign| match sign.sign_source() {
                SignatureSource::RefIndex(index) => *index == SIGNATURE_SOURCE_REFINDEX_OWNER,
                _ => false,
            })
        })
    }

    pub fn miner_signature(&self) -> Option<&Signature> {
        self.as_ref().signs().desc_signs().and_then(|signs| {
            signs.iter().find(|sign| match sign.sign_source() {
                SignatureSource::Object(link) => self.miner().eq(&link.obj_id),
                _ => false,
            })
        })
    }

    pub fn is_order(&self) -> bool {
        self.miner().eq(&ObjectId::default())
    }

    // 消费者
    pub fn consumer(&self) -> &ObjectId {
        self.obj.desc().owner().as_ref().unwrap()
    }

    // 提供者
    pub fn miner(&self) -> &ObjectId {
        &self.obj.desc().content().miner
    }

    // 见证方式
    pub fn witness(&self) -> &T {
        &self.obj.desc().content().witness
    }

    pub fn witness_dec_id(&self) -> Option<&ObjectId> {
        self.obj.desc().content().witness_dec_id.as_ref()
    }

    // contract id 就是object id
    pub fn id(&self) -> ObjectId {
        self.obj.desc().object_id()
    }

    // 数据源
    pub fn data_source(&self) -> &DsgDataSource {
        &self.obj.desc().content().data_source
    }

    // 存储介质
    pub fn storage(&self) -> &DsgStorage {
        &self.obj.desc().content().storage
    }

    // 合约开始时间
    pub fn start_at(&self) -> u64 {
        self.obj.desc().content().start_at
    }

    // 合约结束时间
    pub fn end_at(&self) -> u64 {
        self.obj.desc().content().end_at
    }

    // 初始状态
    pub fn initial_state(&self) -> DsgContractStateObject {
        match self.data_source() {
            DsgDataSource::Immutable(chunks) => DsgContractStateObjectRef::new(
                self.id(),
                DsgContractState::DataSourceChanged(DsgDataSourceChangedState {
                    chunks: chunks.clone(),
                }),
            ),
            DsgDataSource::Mutable(_) => {
                DsgContractStateObjectRef::new(self.id(), DsgContractState::Initial)
            }
        }
    }
}

pub struct DsgContractObjectMutRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    obj: &'a mut DsgContractObject<T>,
}

impl<'a, T> From<&'a mut DsgContractObject<T>> for DsgContractObjectMutRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn from(obj: &'a mut DsgContractObject<T>) -> Self {
        Self { obj }
    }
}

impl<'a, T> DsgContractObjectMutRef<'a, T>
where
    T: Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn set_miner(&mut self, miner: ObjectId) -> BuckyResult<()> {
        if !self.obj.desc().content().miner.eq(&ObjectId::default()) {
            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has miner"))
        } else {
            self.obj.desc_mut().content_mut().miner = miner;
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct DsgDataSourceChangedState {
    pub chunks: Vec<ChunkId>,
}

impl TryFrom<&DsgDataSourceChangedState> for protos::DataSourceChangedState {
    type Error = BuckyError;

    fn try_from(rust: &DsgDataSourceChangedState) -> BuckyResult<Self> {
        let mut proto = protos::DataSourceChangedState::new();
        proto.set_chunks(ProtobufCodecHelper::encode_buf_list(&rust.chunks)?);
        Ok(proto)
    }
}

impl TryFrom<protos::DataSourceChangedState> for DsgDataSourceChangedState {
    type Error = BuckyError;

    fn try_from(mut proto: protos::DataSourceChangedState) -> BuckyResult<Self> {
        Ok(Self {
            chunks: ProtobufCodecHelper::decode_buf_list(proto.take_chunks())?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DsgDataSourcePreparedState {
    pub chunks: Vec<ChunkId>,
    pub data_source_stub: ObjectId,
}

impl TryFrom<&DsgDataSourcePreparedState> for protos::DataSourcePreparedState {
    type Error = BuckyError;

    fn try_from(rust: &DsgDataSourcePreparedState) -> BuckyResult<Self> {
        let mut proto = protos::DataSourcePreparedState::new();
        proto.set_chunks(ProtobufCodecHelper::encode_buf_list(&rust.chunks)?);
        proto.set_data_source_stub(rust.data_source_stub.to_vec()?);
        Ok(proto)
    }
}

impl TryFrom<protos::DataSourcePreparedState> for DsgDataSourcePreparedState {
    type Error = BuckyError;

    fn try_from(mut proto: protos::DataSourcePreparedState) -> BuckyResult<Self> {
        Ok(Self {
            chunks: ProtobufCodecHelper::decode_buf_list(proto.take_chunks())?,
            data_source_stub: ProtobufCodecHelper::decode_buf(proto.take_data_source_stub())?,
        })
    }
}

#[derive(Clone, Debug)]
pub enum DsgContractState {
    Initial,
    // app -> dsg service
    // dsg service -> miner
    DataSourceChanged(DsgDataSourceChangedState),
    // dsg -> app
    DataSourcePrepared(DsgDataSourcePreparedState),
    DataSourceSyncing,
    // app -> dsg
    DataSourceStored,
    ContractExecuted,
    ContractBroken,
}

impl TryFrom<&DsgContractState> for protos::ContractState {
    type Error = BuckyError;

    fn try_from(rust: &DsgContractState) -> BuckyResult<Self> {
        let mut proto = protos::ContractState::new();
        match rust {
            DsgContractState::Initial => {
                proto.set_state_type(protos::ContractState_ContractStateType::Initial);
            }
            DsgContractState::DataSourceChanged(changed) => {
                proto.set_state_type(protos::ContractState_ContractStateType::DataSourceChanged);
                proto.set_data_source_changed(protos::DataSourceChangedState::try_from(changed)?);
            }
            DsgContractState::DataSourcePrepared(prepared) => {
                proto.set_state_type(protos::ContractState_ContractStateType::DataSourcePrepared);
                proto
                    .set_data_source_prepared(protos::DataSourcePreparedState::try_from(prepared)?);
            }
            DsgContractState::DataSourceSyncing => {
                proto.set_state_type(protos::ContractState_ContractStateType::DataSourceSyncing);
            }
            DsgContractState::DataSourceStored => {
                proto.set_state_type(protos::ContractState_ContractStateType::DataSourceStored);
            }
            DsgContractState::ContractExecuted => {
                proto.set_state_type(protos::ContractState_ContractStateType::ContractExecuted);
            }
            DsgContractState::ContractBroken => {
                proto.set_state_type(protos::ContractState_ContractStateType::ContractBroken);
            }
        }
        Ok(proto)
    }
}

impl TryFrom<protos::ContractState> for DsgContractState {
    type Error = BuckyError;

    fn try_from(mut proto: protos::ContractState) -> BuckyResult<Self> {
        Ok(match proto.state_type {
            protos::ContractState_ContractStateType::Initial => Self::Initial,
            protos::ContractState_ContractStateType::DataSourceChanged => Self::DataSourceChanged(
                DsgDataSourceChangedState::try_from(proto.take_data_source_changed())?,
            ),
            protos::ContractState_ContractStateType::DataSourcePrepared => {
                Self::DataSourcePrepared(DsgDataSourcePreparedState::try_from(
                    proto.take_data_source_prepared(),
                )?)
            }
            protos::ContractState_ContractStateType::DataSourceSyncing => Self::DataSourceSyncing,
            protos::ContractState_ContractStateType::DataSourceStored => Self::DataSourceStored,
            protos::ContractState_ContractStateType::ContractExecuted => Self::ContractExecuted,
            protos::ContractState_ContractStateType::ContractBroken => Self::ContractBroken,
        })
    }
}

#[derive(Clone)]
pub struct DsgContractStateDesc {
    pub contract: ObjectId,
    pub state: DsgContractState,
}

impl TryFrom<&DsgContractStateDesc> for protos::ContractStateDesc {
    type Error = BuckyError;

    fn try_from(rust: &DsgContractStateDesc) -> BuckyResult<Self> {
        let mut proto = protos::ContractStateDesc::new();
        proto.set_contract(rust.contract.to_vec()?);
        proto.set_state(protos::ContractState::try_from(&rust.state)?);
        Ok(proto)
    }
}

impl TryFrom<protos::ContractStateDesc> for DsgContractStateDesc {
    type Error = BuckyError;

    fn try_from(mut proto: protos::ContractStateDesc) -> BuckyResult<Self> {
        Ok(Self {
            contract: ProtobufCodecHelper::decode_buf(proto.take_contract())?,
            state: DsgContractState::try_from(proto.take_state())?,
        })
    }
}

impl DescContent for DsgContractStateDesc {
    fn obj_type() -> u16 {
        obj_id::CONTRACT_STATE_OBJECT_TYPE
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl_default_protobuf_raw_codec!(DsgContractStateDesc, protos::ContractStateDesc);

#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgContractStateBody {}

impl BodyContent for DsgContractStateBody {}

pub type DsgContractStateObjectType = NamedObjType<DsgContractStateDesc, DsgContractStateBody>;
pub type DsgContractStateObject = NamedObjectBase<DsgContractStateObjectType>;

#[derive(Copy, Clone)]
pub struct DsgContractStateObjectRef<'a> {
    obj: &'a DsgContractStateObject,
}

impl<'a> std::fmt::Debug for DsgContractStateObjectRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DsgContractStateObject{{contract={}, id={}, state={:?}}}",
            self.contract_id(),
            self.id(),
            self.state()
        )
    }
}

impl<'a> std::fmt::Display for DsgContractStateObjectRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DsgContractStateObject{{contract={}, id={}, state={:?}}}",
            self.contract_id(),
            self.id(),
            self.state()
        )
    }
}

impl<'a> DsgContractStateObjectRef<'a> {
    pub fn new(contract: ObjectId, state: DsgContractState) -> DsgContractStateObject {
        let desc = DsgContractStateDesc {
            contract: contract.clone(),
            state,
        };
        let state = NamedObjectBuilder::new(desc, DsgContractStateBody {})
            .dec_id(dsg_dec_id())
            .ref_objects(vec![ObjectLink {
                obj_id: contract.clone(),
                obj_owner: None,
            }])
            .build();
        state
    }

    pub fn id(&self) -> ObjectId {
        self.as_ref().desc().object_id()
    }

    pub fn state(&self) -> &DsgContractState {
        &self.as_ref().desc().content().state
    }

    pub fn next(&self, state: DsgContractState) -> BuckyResult<DsgContractStateObject> {
        let ref_objects = match &self.as_ref().desc().content().state {
            DsgContractState::Initial => match &state {
                DsgContractState::DataSourceChanged(_) => Ok(vec![]),
                DsgContractState::ContractBroken => Ok(vec![]),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "state should be data source changed after initial",
                )),
            },
            DsgContractState::DataSourceChanged(_) => match &state {
                DsgContractState::DataSourcePrepared(_) => Ok(vec![]),
                DsgContractState::ContractBroken => Ok(vec![]),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "state should be data source prepared after data source changed",
                )),
            },
            DsgContractState::DataSourcePrepared(prepared) => match &state {
                DsgContractState::DataSourceSyncing => Ok(vec![prepared.data_source_stub.clone()]),
                DsgContractState::DataSourceStored => Ok(vec![]),
                DsgContractState::ContractBroken => Ok(vec![]),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "state should be data source stored after data source prepared",
                )),
            },
            DsgContractState::DataSourceSyncing => match &state {
                DsgContractState::DataSourceStored => Ok(vec![]),
                DsgContractState::ContractBroken => Ok(vec![]),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "state should be data source changed or excuted after data source stored",
                )),
            },
            DsgContractState::DataSourceStored => match &state {
                DsgContractState::DataSourceChanged(_) => Ok(vec![]),
                DsgContractState::ContractExecuted => Ok(vec![]),
                DsgContractState::ContractBroken => Ok(vec![]),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "state should be data source changed or excuted after data source stored",
                )),
            },
            DsgContractState::ContractExecuted => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "no invalid state after executed",
            )),
            DsgContractState::ContractBroken => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "no invalid state after broken",
            )),
        }?;
        let desc = DsgContractStateDesc {
            contract: self.contract_id().clone(),
            state,
        };

        let state = NamedObjectBuilder::new(desc, DsgContractStateBody {})
            .dec_id(dsg_dec_id())
            .prev(self.id())
            .ref_objects(
                ref_objects
                    .into_iter()
                    .map(|obj_id| ObjectLink {
                        obj_id,
                        obj_owner: None,
                    })
                    .collect(),
            )
            .build();

        Ok(state)
    }

    pub fn contract_id(&self) -> &ObjectId {
        &self.as_ref().desc().content().contract
    }

    pub fn prev_state_id(&self) -> Option<&ObjectId> {
        self.as_ref().desc().prev().as_ref()
    }

    pub fn create_at(&self) -> u64 {
        self.as_ref().desc().create_time()
    }
}

impl<'a> AsRef<DsgContractStateObject> for DsgContractStateObjectRef<'a> {
    fn as_ref(&self) -> &DsgContractStateObject {
        self.obj
    }
}

impl<'a> From<&'a DsgContractStateObject> for DsgContractStateObjectRef<'a> {
    fn from(obj: &'a DsgContractStateObject) -> Self {
        Self { obj }
    }
}

impl<'a, 'b> PartialEq<DsgContractStateObjectRef<'b>> for DsgContractStateObjectRef<'a> {
    fn eq(&self, other: &DsgContractStateObjectRef<'b>) -> bool {
        self.id() == other.id()
    }
}
