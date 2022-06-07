use cyfs_base::*;
use crate::MetaExtensionType;

#[derive(Clone, RawEncode, RawDecode)]
pub struct RentParam {
    pub id: ObjectId
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct NameRentParam {
    pub name_id: String
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct ChangeNameParam {
    pub name: String,
    pub to: NameState
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct StopAuctionParam {
    pub name: String,
    pub stop_block: i64,
    pub starting_price: i64
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BidName {
    pub name: String,
    pub price: i64,
    pub bid_id: ObjectId,
    pub coin_id: u8,
    pub take_effect_block: i64,
    pub rent_price: i64,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct UnionWithdraw {
    pub union_id: ObjectId,
    pub account_id: ObjectId,
    pub ctid: CoinTokenId,
    pub value: i64,
    pub height: i64,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct ExtensionEvent {
    pub extension_type: MetaExtensionType,
    pub data: Vec<u8>,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct NFTStopAuction {
    pub nft_id: ObjectId,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct NFTCancelApplyBuyParam {
    pub nft_id: ObjectId,
    pub user_id: ObjectId,
}

#[repr(u8)]
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub enum EventType {
    Rent = 0,
    ChangeName = 1,
    NameRent = 2,
    BidName = 3,
    StopAuction = 4,
    UnionWithdraw = 5,
    Extension = 6,
    NFTStopAuction = 7,
    NFTCancelApplyBuy = 8,
}

#[derive(RawEncode, RawDecode, Clone)]
pub enum Event {
    Rent(RentParam),
    NameRent(NameRentParam),
    ChangeNameEvent(ChangeNameParam),
    BidName(BidName),
    StopAuction(StopAuctionParam),
    UnionWithdraw(UnionWithdraw),
    Extension(ExtensionEvent),
    NFTStopAuction(NFTStopAuction),
    NFTCancelApplyBuy(NFTCancelApplyBuyParam),
}

pub const EVENT_RESULT_SUCCESS: u8 = 0;
pub const EVENT_RESULT_FAILED: u8 = 1;

#[derive(RawEncode, RawDecode, Clone)]
pub struct EventResult {
    pub status: u8,
    pub data: Vec<u8>,
}

impl EventResult {
    pub fn new(status: u8, data: Vec<u8>) -> Self {
        Self {
            status,
            data
        }
    }
}

#[derive(RawEncode, RawDecode, Clone)]
pub struct EventRecord {
    pub event: Event,
    pub event_result: EventResult,
}

impl EventRecord {
    pub fn new(event: Event, event_result: EventResult) -> Self {
        Self {
            event,
            event_result
        }
    }
}

impl Event {
    pub fn get_type(&self) -> EventType {
        match self {
            Event::Rent(_) => { EventType::Rent },
            Event::ChangeNameEvent(_) => { EventType::ChangeName },
            Event::NameRent(_) => { EventType::NameRent },
            Event::BidName(_) => {EventType::BidName},
            Event::StopAuction(_) => {EventType::StopAuction},
            Event::UnionWithdraw(_) => EventType::UnionWithdraw,
            Event::Extension(_) => EventType::Extension,
            Event::NFTStopAuction(_) => {EventType::NFTStopAuction},
            Event::NFTCancelApplyBuy(_) => {EventType::NFTCancelApplyBuy}
        }
    }

    pub fn get_content(&self) -> BuckyResult<Vec<u8>> {
        self.to_vec()
    }
}
