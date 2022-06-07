use cyfs_base::*;
use cyfs_base_meta::*;
use async_trait::async_trait;
use std::convert::TryFrom;
use primitive_types::H256;

#[derive(sqlx::FromRow)]
pub struct DescExtra {
    pub obj_id: ObjectId,
    pub rent_arrears: i64,
    pub rent_value: i64,
    pub coin_id: u8,
    pub data_len: i64,
    pub rent_arrears_count: i64,
    pub other_charge_balance: i64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum AccountInfo {
    People(PeopleDesc),
    Device(DeviceDesc),
    Group(SimpleGroupDesc),
    Union(UnionAccountDesc),
    MinerGroup(MinerGroup),
}

impl AccountInfo {
    pub fn get_id(&self) -> ObjectId {
        match self {
            Self::People(desc) => {
                desc.calculate_id()
            }
            Self::Device(desc) => {
                desc.calculate_id()
            }
            Self::Group(desc) => {
                desc.calculate_id()
            }
            Self::Union(desc) => {
                desc.calculate_id()
            }
            Self::MinerGroup(group) => {
                group.desc().calculate_id()
            }
        }
    }

    pub fn get_public_key(&self) -> BuckyResult<&PublicKey> {
        match self {
            Self::People(desc) => {
                Ok(desc.public_key())
            }
            Self::Device(desc) => {
                Ok(desc.public_key())
            }
            Self::Group(_) => {
                Err(BuckyError::new(BuckyErrorCode::Failed, "Failed"))
            }
            Self::Union(_) => {
                Err(BuckyError::new(BuckyErrorCode::Failed, "Failed"))
            }
            Self::MinerGroup(_) => {
                Err(BuckyError::new(BuckyErrorCode::Failed, "Failed"))
            }
        }
    }
}

impl TryFrom<TxCaller> for AccountInfo {
    type Error = BuckyError;

    fn try_from(value: TxCaller) -> Result<Self, Self::Error> {
        match value {
            TxCaller::Device(desc) => {
                Ok(AccountInfo::Device(desc))
            }
            TxCaller::People(desc) => {
                Ok(AccountInfo::People(desc))
            }
            TxCaller::Group(desc) => {
                Ok(AccountInfo::Group(desc))
            }
            TxCaller::Union(desc) => {
                Ok(AccountInfo::Union(desc))
            }
            _ => {
                Err(BuckyError::new(BuckyErrorCode::Failed, "Failed"))
            }
        }
    }
}

impl DescExtra {
    pub fn new_desc_rent_state(obj_id: &ObjectId, rent_arrears: i64, rent_arrears_count: i64, rent_value: i64, coin_id: u8, data_len: i64) -> DescExtra {
        DescExtra {
            obj_id: obj_id.clone(),
            rent_arrears,
            rent_value,
            coin_id,
            data_len,
            rent_arrears_count,
            other_charge_balance: 0
        }
    }

    pub fn new_other_charge_balance(obj_id: &ObjectId, other_charge_balance: i64) -> DescExtra {
        DescExtra {
            obj_id: obj_id.clone(),
            rent_arrears: 0,
            rent_value: 0,
            coin_id: 0,
            data_len: 0,
            rent_arrears_count: 0,
            other_charge_balance
        }
    }
}

pub struct NameExtra {
    pub name_id: String,
    pub rent_arrears: i64,
    pub rent_value: i64,
    pub rent_arrears_count: i64,
    pub coin_id: u8,
    pub owner: ObjectId,
    pub buy_coin_id: u8,
    pub buy_price: i64,
}

impl NameExtra {
    pub fn new_name_rent_state(name_id: &str, rent_arrears: i64, rent_arrears_count: i64, rent_value: i64, coin_id: u8, owner: &ObjectId) -> NameExtra {
        NameExtra {
            name_id: name_id.to_owned(),
            rent_arrears,
            rent_arrears_count,
            rent_value,
            coin_id,
            owner: owner.clone(),
            buy_price: 0,
            buy_coin_id: 0
        }
    }

    pub fn new_buy_price(name_id: &str, owner: &ObjectId, buy_coin_id: u8, buy_price: i64) -> NameExtra {
        NameExtra {
            name_id: name_id.to_owned(),
            rent_arrears: 0,
            rent_arrears_count: 0,
            rent_value: 0,
            coin_id: 0,
            owner: owner.clone(),
            buy_price,
            buy_coin_id
        }
    }
}

pub struct UnpaidRecord {
    pub id: u64,
    pub account_id: ObjectId,
    pub to: ObjectId,
    pub record_type: String,
    pub height: u64,
    pub coin_id: CoinTokenId,
    pub amount: u64,
}

#[async_trait]
pub trait State: Send + Sync {
    // 得到最终受益人账号
    // 为防止受益人设置出现环，这里先限定只查询8次
    async fn get_final_benefi(&self, address: &ObjectId) -> BuckyResult<ObjectId>  {
        let mut ret = address.clone();
        let mut num = 0u8;
        loop {
            if num >= 8 {
                break;
            }
            let benefi = self.get_beneficiary(&ret).await?;
            if &ret == &benefi {
                return Ok(benefi);
            } else {
                ret = benefi;
                num += 1;
            }
        }
        Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "get benefi out of 8 limit"))
    }

    async fn being_transaction(&self) -> BuckyResult<()>;
    async fn rollback(&self) -> BuckyResult<()>;
    async fn commit(&self) -> BuckyResult<()>;

    // 每次启动的初始化，可以用来检查数据
    async fn init(&self) -> BuckyResult<()>;

    async fn create_cycle_event_table(&self, cycle: i64) -> BuckyResult<()>;

    async fn config_get(&self, key: &str, default: &str) -> BuckyResult<String>;
    // config相关
    async fn config_set(&self, key: &str, value: &str) -> BuckyResult<()>;

    // 初始化state
    async fn init_genesis(&self, config: &Vec<GenesisCoinConfig>) -> BuckyResult<()>;

    // tx nonce
    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64>;
    async fn inc_nonce(&self, account: &ObjectId) -> BuckyResult<i64>;

    async fn add_account_info(&self, info: &AccountInfo) -> BuckyResult<()>;
    async fn get_account_info(&self, account: &ObjectId) -> BuckyResult<AccountInfo>;

    // 获取权限
    async fn get_account_permission(&self, account: &ObjectId) -> BuckyResult<u32>;

    // coin and token
    async fn get_balance(&self, account: &ObjectId, ctid: &CoinTokenId) -> BuckyResult<i64>;
    async fn modify_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()>;
    async fn inc_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()>;
    async fn dec_balance(&self, ctid: &CoinTokenId, account: &ObjectId, v: i64) -> BuckyResult<()>;
    async fn issue_token(
        &self,
        to: &ObjectId,
        v: u64,
        token_id: &ObjectId) -> BuckyResult<()>;

    // for union account
    async fn get_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId) -> BuckyResult<UnionBalance>;
    async fn get_union_deviation_seq(&self, ctid: &CoinTokenId, union: &ObjectId) -> BuckyResult<i64>;
    async fn update_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, balance: &UnionBalance) -> BuckyResult<()>;
    async fn deposit_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, from: PeerOfUnion, v: i64) -> BuckyResult<()>;
    async fn withdraw_union_balance(&self, ctid: &CoinTokenId, union: &ObjectId, to: PeerOfUnion, withdraw: i64) -> BuckyResult<i64>;
    async fn update_union_deviation(&self, ctid: &CoinTokenId, union: &ObjectId, deviation: i64, seq: i64) -> BuckyResult<()>;

    async fn get_desc_extra(&self, id: &ObjectId) -> BuckyResult<DescExtra>;
    async fn add_or_update_desc_extra(&self, state: &DescExtra) -> BuckyResult<()>;
    async fn add_or_update_desc_rent_state(&self, state: &DescExtra) -> BuckyResult<()>;
    async fn add_or_update_desc_other_charge_balance(&self, state: &DescExtra) -> BuckyResult<()>;
    async fn update_desc_extra(&self, state: &DescExtra) -> BuckyResult<()>;
    async fn drop_desc_extra(&self, obj_id: &ObjectId) -> BuckyResult<()>;

    async fn get_name_extra(&self, id: &str) -> BuckyResult<NameExtra>;
    async fn add_or_update_name_extra(&self, state: &NameExtra) -> BuckyResult<()>;
    async fn add_or_update_name_rent_state(&self, state: &NameExtra) -> BuckyResult<()>;
    async fn add_or_update_name_buy_price(&self, state: &NameExtra) -> BuckyResult<()>;

    //name info
    async fn create_name_info(&self,name:&str,info:&NameInfo)->BuckyResult<()>;

    async fn get_name_info(
        &self,
        name: &str) -> BuckyResult<Option<(NameInfo,NameState)>>;
    async fn get_owned_names(&self, owner: &ObjectId)->BuckyResult<Vec<String>>;

    async fn get_name_state(&self, name: &str) -> BuckyResult<NameState>;

    async fn update_name_info(
        &self,
        name: &str,
        info: &NameInfo
    ) -> BuckyResult<()>;

    async fn update_name_state(&self, name:&str, state:NameState) -> BuckyResult<()>;
    async fn update_name_rent_arrears(&self, name:&str, rent_arrears: i64) -> BuckyResult<()>;

    //desc
    async fn create_obj_desc (&self, objid: &ObjectId, desc:&SavedMetaObject) -> BuckyResult<()>;

    async fn get_obj_desc(
        &self,
        objid: &ObjectId
    ) -> BuckyResult<SavedMetaObject>;
    async fn update_obj_desc(
        &self,
        objid: &ObjectId,
        desc: &SavedMetaObject, flags:u8) -> BuckyResult<()>;
    async fn drop_desc(&self, obj_id: &ObjectId) -> BuckyResult<()>;

    async fn add_or_update_cycle_event(&self, key: &str, event: &Event, cycle: i64, start_height: i64) -> BuckyResult<()>;
    async fn get_cycle_events(&self, offset: i64, cycle: i64) -> BuckyResult<Vec<(String, i64, Event)>>;
    async fn get_all_cycle_events(&self, cycle: i64) -> BuckyResult<Vec<(String, i64, Event)>>;
    async fn get_cycle_event_by_key(&self, key: &str, cycle: i64) -> BuckyResult<Event>;
    async fn get_cycle_event_by_key2(&self, key: &str, cycle: i64) -> BuckyResult<(i64, Event)>;
    async fn drop_cycle_event(&self, key: &str, cycle: i64) -> BuckyResult<()>;
    async fn drop_all_cycle_events(&self, cycle: i64) -> BuckyResult<()>;

    // key字段是为了方便后续update一个尚未执行的Event
    // 插入或更新一个要执行的Event，主键是(eventtype, key, height)的三元组
    async fn add_or_update_event(&self, key: &str, event: Event, height: i64) -> BuckyResult<()>;
    // 通过type和height，获取这个height上应该执行的Event列表，一般用于执行Event用
    async fn get_event(&self, event_type: EventType, height: i64) -> BuckyResult<Vec<Event>>;

    // 通过type和key，获取Event列表和应该执行的height，一般用于Update用
    async fn get_event_by_key(&self, key: &str, event_type: EventType) -> BuckyResult<Vec<(Event, i64)>>;
    // 清理所有小于等于height的Event，这个函数会被executer在每块所有Event执行完毕后调用
    async fn drop_event(&self, height: i64) -> BuckyResult<()>;

    async fn add_or_update_once_event(&self, key: &str, event: &Event, height: i64) -> BuckyResult<()>;
    async fn get_once_events(&self, height: i64) -> BuckyResult<Vec<Event>>;
    async fn get_once_event_by_key(&self, key: &str) -> BuckyResult<Event>;
    async fn drop_once_event(&self, height: i64) -> BuckyResult<()>;
    async fn drop_once_event_by_key(&self, key: &str) -> BuckyResult<()>;

    // async fn add_service(&self, service_id: &ObjectId, service_status: u8, service: Vec<u8>) -> BuckyResult<()>;
    // async fn update_service_status(&self, service_id: &ObjectId, service_status: u8) -> BuckyResult<()>;
    // async fn get_service(&self, service_id: &ObjectId) -> BuckyResult<(u8, Vec<u8>)>;
    // async fn add_contract(&self, contract_id: &ObjectId, service_id: &ObjectId, buyer_id: &ObjectId, auth_type: u8, contract: Vec<u8>, auth_list: Vec<u8>) -> BuckyResult<()>;
    // async fn update_contract(&self, contract_id: &ObjectId, auth_type: u8, auth_list: Vec<u8>) -> BuckyResult<()>;
    // async fn get_contract(&self, contract_id: &ObjectId) -> BuckyResult<(Vec<u8>, u8, Vec<u8>)>;
    // async fn get_contract_by_buyer(&self, service_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<(Vec<u8>, u8, Vec<u8>)>;

    async fn create_subchain_withdraw_record(&self, subchain_id: &ObjectId, withdraw_tx_id: &ObjectId, record: Vec<u8>) -> BuckyResult<()>;
    async fn update_subchain_withdraw_record(&self, subchain_id: &ObjectId, withdraw_tx_id: &ObjectId, record: Vec<u8>) -> BuckyResult<()>;
    async fn get_subchain_withdraw_record(&self, subchain_id: &ObjectId, withdraw_tx_id: &ObjectId) -> BuckyResult<Vec<u8>>;

    async fn add_unpaid_record(&self, record: &UnpaidRecord) -> BuckyResult<()>;
    async fn drop_unpaid_record(&self, id: u64, coin_id: &CoinTokenId) -> BuckyResult<()>;
    async fn get_unpaid_records(&self, account_id: &ObjectId, coin_id: &CoinTokenId) -> BuckyResult<Vec<UnpaidRecord>>;

    async fn get_cycles(&self) -> BuckyResult<Vec<i64>>;
    async fn delete_cycle(&self, cycle: i64) -> BuckyResult<()>;

    // evm所需接口
    // address指定的账户是否存在，账户没有数据(desc或者code)和余额就当作不存在
    async fn account_exists(&self, address: &ObjectId) -> BuckyResult<bool>;
    // 返回address上存储的code，没有则返回空Vec
    async fn code(&self, address: &ObjectId) -> BuckyResult<Vec<u8>>;
    // 通过Address和index获取当前存储的值
    async fn storage(&self, address: &ObjectId, index: &H256) -> BuckyResult<H256>;
    // 根据address和index写入存储的值
    async fn set_storage(&self, address: &ObjectId, index: &H256, value: H256) -> BuckyResult<()>;
    // 设置账户的nonce
    async fn set_nonce(&self, address:&ObjectId, nonce: i64) -> BuckyResult<()>;
    // 给账户设置code
    async fn set_code(&self, address: &ObjectId, code: Vec<u8>) -> BuckyResult<()>;
    // 重置账户的所有storage
    async fn reset_storage(&self, address: &ObjectId) -> BuckyResult<()>;
    // 移除某个特定的storage，减少存储占用（非必须接口）
    async fn remove_storage(&self, address: &ObjectId, index: &H256) -> BuckyResult<()>;
    // 销毁某个合约，也同时销毁这个合约的code，storage，减少存储占用
    async fn delete_contract(&self, address: &ObjectId) -> BuckyResult<()>;

    // evm的log是需要存储到state里的，后续会有查询需求：
    // 一个Log由Address，Hash[], Data组成，Address和Hash[]可以用来查询，Hash[]不超过4
    // 全0的H256表示undefined，忽略这个选项查询
    async fn set_log(&self, address: &ObjectId, block_number: i64, topics: &[H256], data: Vec<u8>) -> BuckyResult<()>;
    // from 为0，表示不设置查询下限
    // to为0，表示不设置查询上限
    async fn get_log(&self, address: &ObjectId, from: i64, to: i64, topics: &[Option<H256>]) -> BuckyResult<Vec<(Vec<H256>, Vec<u8>)>>;

    // 设置账户受益人
    async fn set_beneficiary(&self, address: &ObjectId, beneficiary: &ObjectId) -> BuckyResult<()>;
    // 查询设置过的受益人，如果一个账户没有设置过受益人，返回address自身
    async fn get_beneficiary(&self, address: &ObjectId) -> BuckyResult<ObjectId>;

    async fn nft_create(&self, object_id: &ObjectId, desc: &NFTDesc, name: &str, state: &NFTState) -> BuckyResult<()>;
    async fn nft_set_name(&self, nft_id: &ObjectId, name: &str) -> BuckyResult<()>;
    async fn nft_get(&self, object_id: &ObjectId) -> BuckyResult<(NFTDesc, String, NFTState)>;
    async fn nft_update_state(&self, object_id: &ObjectId, state: &NFTState) -> BuckyResult<()>;
    async fn nft_add_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()>;
    async fn nft_get_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>>;
    async fn nft_get_apply_buy_list(&self, nft_id: &ObjectId, offset: i64, length: i64) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>>;
    async fn nft_get_apply_buy_count(&self, nft_id: &ObjectId) -> BuckyResult<i64>;
    async fn nft_remove_all_apply_buy(&self, nft_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_remove_apply_buy(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_add_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()>;
    async fn nft_get_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<Option<(u64, CoinTokenId)>>;
    async fn nft_get_bid_list(&self, nft_id: &ObjectId, offset: i64, length: i64) -> BuckyResult<Vec<(ObjectId, u64, CoinTokenId)>>;
    async fn nft_get_bid_count(&self, nft_id: &ObjectId) -> BuckyResult<i64>;
    async fn nft_remove_all_bid(&self, nft_id: &ObjectId) -> BuckyResult<()>;
    async fn nft_remove_bid(&self, nft_id: &ObjectId, buyer_id: &ObjectId) -> BuckyResult<()>;
}


