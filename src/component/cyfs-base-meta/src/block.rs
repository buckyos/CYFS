use sha2::{Digest, Sha256};
// use merkletree::merkle::{MerkleTree};
use crate::*;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_core::CoreObjectType;

pub type TxHash = ObjectId;
pub type StateHash = HashValue;

pub type BlockHash = ObjectId;

type ReceiptHash = HashValue;
type TransactionHash = HashValue;

pub trait BlockDescTrait {
    fn hash(&self) -> BlockHash;
    fn hash_str(&self) -> String;
    fn pre_block_hash(&self) -> &BlockHash;
    fn pre_block_hash_str(&self) -> String;
    fn number(&self) -> i64;
    fn coinbase(&self) -> &ObjectId;
    fn is_pre_block_of(&self, other: &Self) -> bool;
    fn state_hash(&self) -> &HashValue;
    fn transactions_hash(&self) -> &HashValue;
    fn receipts_hash(&self) -> &HashValue;
    fn event_records_hash(&self) -> &HashValue;
}

pub trait BlockBodyTrait {
    type Receipt;
    fn transactions(&self) -> &Vec<MetaTx>;
    fn receipts(&self) -> Vec<Self::Receipt>;
    // return index
    fn add_transaction(&mut self, tx: MetaTx) -> Result<usize, u32>;
    fn add_receipts(&mut self, receipts: Vec<Self::Receipt>) -> Result<(), u32>;
    fn add_event_record(&mut self, event_record: EventRecord);
    fn set_event_records(&mut self, event_records: Vec<EventRecord>);
    fn event_records(&self) -> &Vec<EventRecord>;
}

#[async_trait]
pub trait BlockTrait {
    type BlockDesc;
    type BlockBody;
    type BlockBuilder;
    type Receipt;
    fn new(
        coinbase: ObjectId,
        pre_block: Option<&Self::BlockDesc>,
        state_hash: StateHash,
        body: Self::BlockBody,
    ) -> BuckyResult<Self::BlockBuilder>;
    fn new2(
        src_desc: &Self::BlockDesc,
        state_hash: StateHash,
        body: Self::BlockBody,
    ) -> BuckyResult<Self::BlockBuilder>;
    fn header(&self) -> &Self::BlockDesc;
    fn transactions(&self) -> &Vec<MetaTx>;
    fn receipts(&self) -> Vec<Self::Receipt>;
    fn event_records(&self) -> &Vec<EventRecord>;
    async fn sign(
        &mut self,
        private_key: PrivateKey,
        sign_source: &SignatureSource,
    ) -> BuckyResult<Signature>;
    fn version(&self) -> u64;
}

// pub type BlockDesc = BlockDescV2;
// pub type BlockType = BlockTypeV2;
// pub type BlockId = BlockIdV2;
// pub type Block = BlockV2;
// pub type BlockBuilder = BlockBuilderV2;
// pub type BlockDescContent = BlockDescContentV2;
// pub type BlockBody = BlockBodyV2;

type TxList = Vec<MetaTx>;
type ReceiptList = Vec<Receipt>;
type EventRecordList = Vec<EventRecord>;

// type MerkleHash = GenericArray<u8, <Sha256 as Digest>::OutputSize>;

#[derive(Clone, RawEncode, RawDecode)]
pub struct BlockDescContentV1 {
    pub number: i64,
    pub coinbase: ObjectId,
    pub state_hash: StateHash,
    pub pre_block_hash: BlockHash,
    //tx_merkle_root: MerkleHash,
    pub transactions_hash: TransactionHash,
    pub receipts_hash: ReceiptHash,
    // pub event_records_hash: HashValue,
}

impl BlockDescContentV1 {
    pub fn new(coinbase: ObjectId, pre_block: Option<&BlockDescV1>) -> Self {
        let mut desc_content = BlockDescContentV1 {
            number: 0,
            coinbase,
            state_hash: HashValue::default(),
            pre_block_hash: BlockHash::default(),
            transactions_hash: HashValue::default(),
            receipts_hash: HashValue::default(),
            // event_records_hash: HashValue::default()
        };

        if let Some(pre_header) = pre_block {
            desc_content.number = pre_header.number() + 1;
            desc_content.pre_block_hash = pre_header.calculate_id();
        }

        desc_content
    }
}

impl DescContent for BlockDescContentV1 {
    fn obj_type() -> u16 {
        CoreObjectType::BlockV1 as u16
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BlockBodyV1 {
    pub transactions: TxList,
    pub receipts: Vec<ReceiptV1>,
    // pub event_records: EventRecordList,
}

impl BodyContent for BlockBodyV1 {}

impl BlockBodyV1 {
    pub fn new() -> Self {
        BlockBodyV1 {
            transactions: vec![],
            receipts: vec![],
        }
    }
}

impl BlockBodyTrait for BlockBodyV1 {
    type Receipt = ReceiptV1;
    fn transactions(&self) -> &Vec<MetaTx> {
        &self.transactions
    }

    fn receipts(&self) -> Vec<Self::Receipt> {
        self.receipts.clone()
    }
    // return index
    fn add_transaction(&mut self, tx: MetaTx) -> Result<usize, u32> {
        self.transactions.push(tx);
        Ok(self.transactions.len() - 1)
    }

    fn add_receipts(&mut self, receipts: Vec<Self::Receipt>) -> Result<(), u32> {
        for receipt in receipts {
            self.receipts.push(receipt);
        }
        Ok(())
    }

    fn add_event_record(&mut self, _event_record: EventRecord) {
        // self.event_records.push(event_record);
    }

    fn set_event_records(&mut self, _event_records: Vec<EventRecord>) {
        // self.event_records = event_records;
    }

    fn event_records(&self) -> &Vec<EventRecord> {
        unimplemented!()
    }
}

pub type BlockDescV1 = NamedObjectDesc<BlockDescContentV1>;
pub type BlockTypeV1 = NamedObjType<BlockDescContentV1, BlockBodyV1>;
pub type BlockIdV1 = NamedObjectId<BlockTypeV1>;
pub type BlockV1 = NamedObjectBase<BlockTypeV1>;
pub type BlockBuilderV1 = NamedObjectBuilder<BlockDescContentV1, BlockBodyV1>;

impl BlockDescTrait for NamedObjectDesc<BlockDescContentV1> {
    fn hash(&self) -> BlockHash {
        self.calculate_id()
    }

    fn hash_str(&self) -> String {
        self.calculate_id().to_string()
    }

    fn pre_block_hash(&self) -> &BlockHash {
        &self.content().pre_block_hash
    }

    fn pre_block_hash_str(&self) -> String {
        self.content().pre_block_hash.to_hex().unwrap()
    }

    fn number(&self) -> i64 {
        self.content().number
    }

    fn coinbase(&self) -> &ObjectId {
        &self.content().coinbase
    }

    fn is_pre_block_of(&self, other: &Self) -> bool {
        return (self.content().number == other.content().number + 1)
            && (self.calculate_id().eq(&other.content().pre_block_hash));
    }

    fn state_hash(&self) -> &HashValue {
        &self.content().state_hash
    }

    fn transactions_hash(&self) -> &HashValue {
        &self.content().transactions_hash
    }

    fn receipts_hash(&self) -> &HashValue {
        &self.content().receipts_hash
    }

    fn event_records_hash(&self) -> &HashValue {
        unimplemented!()
    }
}

#[async_trait]
impl BlockTrait for NamedObjectBase<BlockTypeV1> {
    type BlockDesc = BlockDescV1;
    type BlockBody = BlockBodyV1;
    type BlockBuilder = BlockBuilderV1;
    type Receipt = ReceiptV1;

    fn new(
        coinbase: ObjectId,
        pre_block: Option<&Self::BlockDesc>,
        state_hash: StateHash,
        body: Self::BlockBody,
    ) -> BuckyResult<Self::BlockBuilder> {
        let mut transactions_hasher = Sha256::new();
        for tx in &body.transactions {
            transactions_hasher.input(tx.desc().calculate_id().to_string())
        }
        let transactions_hash = HashValue::from(transactions_hasher.result());

        let mut receipts_hasher = Sha256::new();
        for receipt in &body.receipts {
            receipts_hasher.input(receipt.to_vec()?);
        }
        let receipts_hash = HashValue::from(receipts_hasher.result());

        let mut header = BlockDescContentV1 {
            number: 0,
            coinbase,
            state_hash,
            pre_block_hash: BlockHash::default(),
            transactions_hash,
            receipts_hash,
            // event_records_hash
        };
        if let Some(pre_header) = pre_block {
            header.number = pre_header.content().number + 1;
            header.pre_block_hash = pre_header.calculate_id();
        }

        Ok(BlockBuilderV1::new(header, body))
    }

    fn new2(
        src_desc: &Self::BlockDesc,
        state_hash: StateHash,
        body: Self::BlockBody,
    ) -> BuckyResult<Self::BlockBuilder> {
        let mut transactions_hasher = Sha256::new();
        for tx in &body.transactions {
            transactions_hasher.input(tx.desc().calculate_id().to_string())
        }
        let transactions_hash = HashValue::from(transactions_hasher.result());

        let mut receipts_hasher = Sha256::new();
        for receipt in &body.receipts {
            receipts_hasher.input(receipt.to_vec()?);
        }
        let receipts_hash = HashValue::from(receipts_hasher.result());

        let header = BlockDescContentV1 {
            number: src_desc.number(),
            coinbase: src_desc.coinbase().clone(),
            state_hash,
            pre_block_hash: src_desc.pre_block_hash().clone(),
            transactions_hash,
            receipts_hash,
            // event_records_hash
        };
        let builder = BlockBuilderV1::new(header, body).create_time(src_desc.create_time());
        Ok(builder)
    }

    fn header(&self) -> &Self::BlockDesc {
        self.desc()
    }

    fn transactions(&self) -> &Vec<MetaTx> {
        &self.body().as_ref().unwrap().content().transactions
    }

    fn receipts(&self) -> Vec<Self::Receipt> {
        self.body().as_ref().unwrap().content().receipts.clone()
    }

    fn event_records(&self) -> &Vec<EventRecord> {
        unreachable!()
    }

    async fn sign(
        &mut self,
        private_key: PrivateKey,
        sign_source: &SignatureSource,
    ) -> BuckyResult<Signature> {
        let signer = RsaCPUObjectSigner::new(private_key.public(), private_key);
        let sign = sign_named_object_desc(&signer, self, sign_source).await?;
        self.signs_mut().push_desc_sign(sign.clone());
        Ok(sign)
    }

    fn version(&self) -> u64 {
        1
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BlockDescContentV2 {
    pub number: i64,
    pub coinbase: ObjectId,
    pub state_hash: StateHash,
    pub pre_block_hash: BlockHash,
    //tx_merkle_root: MerkleHash,
    pub transactions_hash: TransactionHash,
    pub receipts_hash: ReceiptHash,
    pub event_records_hash: HashValue,
}

impl BlockDescContent {
    pub fn new(coinbase: ObjectId, pre_block: Option<&BlockDesc>) -> Self {
        if let Some(pre_header) = pre_block {
            BlockDescContent::V2(BlockDescContentV2 {
                number: pre_header.number() + 1,
                coinbase,
                state_hash: HashValue::default(),
                pre_block_hash: pre_header.hash(),
                transactions_hash: HashValue::default(),
                receipts_hash: HashValue::default(),
                event_records_hash: HashValue::default(),
            })
        } else {
            BlockDescContent::V2(BlockDescContentV2 {
                number: 0,
                coinbase,
                state_hash: HashValue::default(),
                pre_block_hash: BlockHash::default(),
                transactions_hash: HashValue::default(),
                receipts_hash: HashValue::default(),
                event_records_hash: HashValue::default(),
            })
        }
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub enum BlockDescContent {
    V1(BlockDescV1),
    V2(BlockDescContentV2),
}

impl DescContent for BlockDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::BlockV2 as u16
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BlockBodyV2 {
    pub transactions: TxList,
    pub receipts: Vec<ReceiptV2>,
    pub event_records: EventRecordList,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BlockBodyV3 {
    pub transactions: TxList,
    pub receipts: ReceiptList,
    pub event_records: EventRecordList,
}

#[derive(Clone, RawEncode, RawDecode)]
pub enum BlockBody {
    V1(BlockV1, Vec<Receipt>),
    V2(BlockBodyV2),
    V3(BlockBodyV3),
}

impl BodyContent for BlockBody {}

impl BlockBody {
    pub fn new() -> Self {
        BlockBody::V3(BlockBodyV3 {
            transactions: vec![],
            receipts: vec![],
            event_records: vec![],
        })
    }
}

impl BlockBodyTrait for BlockBody {
    type Receipt = crate::Receipt;
    fn transactions(&self) -> &Vec<MetaTx> {
        match self {
            BlockBody::V1(body, _) => body.transactions(),
            BlockBody::V2(body) => &body.transactions,
            BlockBody::V3(body) => &body.transactions,
        }
    }

    fn receipts(&self) -> Vec<Self::Receipt> {
        match self {
            BlockBody::V1(_, receipts) => receipts.clone(),
            BlockBody::V2(body) => body.receipts.iter().map(|r| r.into()).collect(),
            BlockBody::V3(body) => body.receipts.clone(),
        }
    }
    // return index
    fn add_transaction(&mut self, tx: MetaTx) -> Result<usize, u32> {
        match self {
            BlockBody::V1(body, _) => body
                .body_mut()
                .as_mut()
                .unwrap()
                .content_mut()
                .add_transaction(tx),
            BlockBody::V2(body) => {
                body.transactions.push(tx);
                Ok(body.transactions.len() - 1)
            }
            BlockBody::V3(body) => {
                body.transactions.push(tx);
                Ok(body.transactions.len() - 1)
            }
        }
    }

    fn add_receipts(&mut self, receipts: Vec<Receipt>) -> Result<(), u32> {
        match self {
            BlockBody::V3(body) => {
                for receipt in receipts {
                    body.receipts.push(receipt);
                }
                Ok(())
            }
            _ => {
                unreachable!()
            }
        }
    }

    fn add_event_record(&mut self, event_record: EventRecord) {
        match self {
            BlockBody::V3(body) => {
                body.event_records.push(event_record);
            }
            _ => {
                unreachable!()
            }
        }
    }

    fn set_event_records(&mut self, event_records: Vec<EventRecord>) {
        match self {
            BlockBody::V3(body) => {
                body.event_records = event_records;
            }
            _ => {
                unreachable!()
            }
        }
    }

    fn event_records(&self) -> &Vec<EventRecord> {
        match self {
            BlockBody::V3(body) => &body.event_records,
            _ => {
                unreachable!()
            }
        }
    }
}

pub type BlockDesc = NamedObjectDesc<BlockDescContent>;
pub type BlockType = NamedObjType<BlockDescContent, BlockBody>;
pub type BlockId = NamedObjectId<BlockType>;
pub type Block = NamedObjectBase<BlockType>;
pub type BlockBuilder = NamedObjectBuilder<BlockDescContent, BlockBody>;

impl BlockDescTrait for NamedObjectDesc<BlockDescContent> {
    fn hash(&self) -> BlockHash {
        match self.content() {
            BlockDescContent::V1(desc) => desc.calculate_id(),
            BlockDescContent::V2(_) => self.calculate_id(),
        }
    }

    fn hash_str(&self) -> String {
        self.hash().to_hex().unwrap()
    }

    fn pre_block_hash(&self) -> &BlockHash {
        match self.content() {
            BlockDescContent::V1(desc) => desc.pre_block_hash(),
            BlockDescContent::V2(desc) => &desc.pre_block_hash,
        }
    }

    fn pre_block_hash_str(&self) -> String {
        match self.content() {
            BlockDescContent::V1(desc) => desc.pre_block_hash_str(),
            BlockDescContent::V2(desc) => desc.pre_block_hash.to_hex().unwrap(),
        }
    }

    fn number(&self) -> i64 {
        match self.content() {
            BlockDescContent::V1(desc) => desc.number(),
            BlockDescContent::V2(desc) => desc.number,
        }
    }

    fn coinbase(&self) -> &ObjectId {
        match self.content() {
            BlockDescContent::V1(desc) => desc.coinbase(),
            BlockDescContent::V2(desc) => &desc.coinbase,
        }
    }

    fn is_pre_block_of(&self, other: &Self) -> bool {
        return (self.number() == other.number() + 1)
            && (self.calculate_id().eq(other.pre_block_hash()));
    }

    fn state_hash(&self) -> &HashValue {
        match self.content() {
            BlockDescContent::V1(desc) => desc.state_hash(),
            BlockDescContent::V2(desc) => &desc.state_hash,
        }
    }

    fn transactions_hash(&self) -> &HashValue {
        match self.content() {
            BlockDescContent::V1(desc) => desc.transactions_hash(),
            BlockDescContent::V2(desc) => &desc.transactions_hash,
        }
    }

    fn receipts_hash(&self) -> &HashValue {
        match self.content() {
            BlockDescContent::V1(desc) => desc.receipts_hash(),
            BlockDescContent::V2(desc) => &desc.receipts_hash,
        }
    }

    fn event_records_hash(&self) -> &HashValue {
        match self.content() {
            BlockDescContent::V2(desc) => &desc.event_records_hash,
            _ => {
                unreachable!()
            }
        }
    }
}

#[async_trait]
impl BlockTrait for NamedObjectBase<BlockType> {
    type BlockDesc = BlockDesc;
    type BlockBody = BlockBody;
    type BlockBuilder = BlockBuilder;
    type Receipt = crate::Receipt;

    fn new(
        coinbase: ObjectId,
        pre_block: Option<&Self::BlockDesc>,
        state_hash: StateHash,
        body: Self::BlockBody,
    ) -> BuckyResult<Self::BlockBuilder> {
        let mut transactions_hasher = Sha256::new();
        for tx in body.transactions() {
            transactions_hasher.input(tx.desc().calculate_id().to_string())
        }
        let transactions_hash = HashValue::from(transactions_hasher.result());

        let mut receipts_hasher = Sha256::new();
        for receipt in body.receipts() {
            receipts_hasher.input(receipt.to_vec()?);
        }
        let receipts_hash = HashValue::from(receipts_hasher.result());

        let mut event_records_haser = Sha256::new();
        for record in body.event_records() {
            event_records_haser.input(record.to_vec()?);
        }
        let event_records_hash = HashValue::from(event_records_haser.result());

        let header = if let Some(pre_header) = pre_block {
            BlockDescContent::V2(BlockDescContentV2 {
                number: pre_header.number() + 1,
                coinbase,
                state_hash,
                pre_block_hash: pre_header.hash(),
                transactions_hash,
                receipts_hash,
                event_records_hash,
            })
        } else {
            BlockDescContent::V2(BlockDescContentV2 {
                number: 0,
                coinbase,
                state_hash,
                pre_block_hash: BlockHash::default(),
                transactions_hash,
                receipts_hash,
                event_records_hash,
            })
        };

        Ok(BlockBuilder::new(header, body))
    }

    fn new2(
        src_desc: &Self::BlockDesc,
        state_hash: StateHash,
        body: Self::BlockBody,
    ) -> BuckyResult<Self::BlockBuilder> {
        let mut transactions_hasher = Sha256::new();
        for tx in body.transactions() {
            transactions_hasher.input(tx.desc().calculate_id().to_string())
        }
        let transactions_hash = HashValue::from(transactions_hasher.result());

        let mut receipts_hasher = Sha256::new();
        for receipt in body.receipts() {
            receipts_hasher.input(receipt.to_vec()?);
        }
        let receipts_hash = HashValue::from(receipts_hasher.result());

        let mut event_records_haser = Sha256::new();
        for record in body.event_records() {
            event_records_haser.input(record.to_vec()?);
        }
        let event_records_hash = HashValue::from(event_records_haser.result());

        let header = BlockDescContent::V2(BlockDescContentV2 {
            number: src_desc.number(),
            coinbase: src_desc.coinbase().clone(),
            state_hash,
            pre_block_hash: src_desc.pre_block_hash().clone(),
            transactions_hash,
            receipts_hash,
            event_records_hash,
        });
        let builder = BlockBuilder::new(header, body).create_time(src_desc.create_time());
        Ok(builder)
    }

    fn header(&self) -> &BlockDesc {
        self.desc()
    }

    fn transactions(&self) -> &Vec<MetaTx> {
        &self.body().as_ref().unwrap().content().transactions()
    }

    fn receipts(&self) -> Vec<Self::Receipt> {
        self.body().as_ref().unwrap().content().receipts()
    }

    fn event_records(&self) -> &Vec<EventRecord> {
        &self.body().as_ref().unwrap().content().event_records()
    }

    async fn sign(
        &mut self,
        private_key: PrivateKey,
        sign_source: &SignatureSource,
    ) -> BuckyResult<Signature> {
        let signer = RsaCPUObjectSigner::new(private_key.public(), private_key);
        let sign = sign_named_object_desc(&signer, self, sign_source).await?;
        self.signs_mut().push_desc_sign(sign.clone());
        Ok(sign)
    }

    fn version(&self) -> u64 {
        match self.desc().content() {
            BlockDescContent::V1(_) => 1,
            BlockDescContent::V2(_) => 2,
        }
    }
}
