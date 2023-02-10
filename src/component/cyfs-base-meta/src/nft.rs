use cyfs_base::*;
use serde::{Serialize, Deserialize};
use cyfs_core::NFTListDesc;

#[derive(RawEncode, RawDecode, Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum NFTState {
    Normal,
    Auctioning((u64, CoinTokenId, u64)),
    Selling((u64, CoinTokenId, u64)),
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub enum NFTDesc {
    FileDesc(FileDesc),
    FileDesc2((FileDesc, Option<ObjectId>)),
    ListDesc(NFTListDesc)
}

impl NFTDesc {
    pub fn nft_id(&self) -> ObjectId {
        match self {
            NFTDesc::FileDesc(desc) => {
                desc.calculate_id()
            }
            NFTDesc::ListDesc(desc) => {
                desc.calculate_id()
            }
            NFTDesc::FileDesc2((desc, _)) => {
                desc.calculate_id()
            }
        }
    }

    pub fn nft_label(&self) -> HashValue {
        match self {
            NFTDesc::FileDesc(desc) => {
                desc.content().hash().clone()
            }
            NFTDesc::ListDesc(desc) => {
                HashValue::try_from(desc.calculate_id().as_slice()).unwrap()
            }
            NFTDesc::FileDesc2((desc, _)) => {
                desc.content().hash().clone()
            }
        }
    }

    pub fn nft_create_time(&self) -> u64 {
        match self {
            NFTDesc::FileDesc(desc) => {
                desc.create_time()
            }
            NFTDesc::ListDesc(desc) => {
                desc.create_time()
            }
            NFTDesc::FileDesc2((desc, _)) => {
                desc.create_time()
            }
        }
    }

    pub fn owner_id(&self) -> &Option<ObjectId> {
        match self {
            NFTDesc::FileDesc(desc) => {
                desc.owner()
            }
            NFTDesc::ListDesc(desc) => {
                desc.owner()
            }
            NFTDesc::FileDesc2((desc, _)) => {
                desc.owner()
            }
        }
    }

    pub fn author_id(&self) -> &Option<ObjectId> {
        match self {
            NFTDesc::FileDesc(desc) => {
                if desc.author().is_some() {
                    desc.author()
                } else {
                    desc.owner()
                }
            }
            NFTDesc::ListDesc(desc) => {
                if desc.author().is_some() {
                    desc.author()
                } else {
                    desc.owner()
                }
            }
            NFTDesc::FileDesc2((desc, _)) => {
                if desc.author().is_some() {
                    desc.author()
                } else {
                    desc.owner()
                }
            }
        }
    }

    pub fn parent_id(&self) -> Option<ObjectId> {
        match self {
            NFTDesc::FileDesc(_) => {
                None
            }
            NFTDesc::FileDesc2((_, parent_id)) => {
                parent_id.clone()
            }
            NFTDesc::ListDesc(_) => {
                None
            }
        }
    }

    pub fn sub_list(&self) -> Option<Vec<ObjectId>> {
        match self {
            NFTDesc::FileDesc(_) => {
                None
            }
            NFTDesc::FileDesc2(_) => {
                None
            }
            NFTDesc::ListDesc(sub_list) => {
                Some(sub_list.content().nft_list.iter().map(|item| item.calculate_id()).collect())
            }
        }
    }
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTCreateTx {
    pub desc: NFTDesc,
    pub name: String,
    pub state: NFTState,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTCreateTx2 {
    pub desc: NFTDesc,
    pub name: String,
    pub state: NFTState,
    pub sub_names: Vec<String>,
    pub sub_states: Vec<NFTState>,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTAuctionTx {
    pub nft_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
    pub duration_block_num: u64,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTBidTx {
    pub nft_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTBuyTx {
    pub nft_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTSellTx {
    pub nft_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
    pub duration_block_num: u64,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTSellTx2 {
    pub nft_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
    pub sub_sell_infos: Vec<(CoinTokenId, u64)>
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTCancelSellTx {
    pub nft_id: ObjectId,
}

// 求购
#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTApplyBuyTx {
    pub nft_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTCancelApplyBuyTx {
    pub nft_id: ObjectId,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTAgreeApplyTx {
    pub nft_id: ObjectId,
    pub user_id: ObjectId,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTLikeTx {
    pub nft_id: ObjectId,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTSetNameTx {
    pub nft_id: ObjectId,
    pub name: String,
}

#[derive(RawEncode, RawDecode, Clone, Debug)]
pub struct NFTTransTx {
    pub nft_id: ObjectId,
    pub to: ObjectId,
    pub nft_cached: Option<ObjectId>,
}

#[derive(Serialize, Deserialize)]
pub struct NFTData {
    pub nft_id: String,
    pub create_time: u64,
    pub beneficiary: String,
    pub owner_id: String,
    pub author_id: String,
    pub name: String,
    pub reward_amount: i64,
    pub like_count: i64,
    pub state: NFTState,
    pub block_number: i64,
    pub parent_id: Option<String>,
    pub sub_list: Option<Vec<String>>,
    pub price: u64,
    pub coin_id: CoinTokenId,
}

#[derive(Serialize, Deserialize)]
pub struct NFTTransferRecord {
    pub nft_id: String,
    pub create_time: u64,
    pub owner_id: String,
    pub author_id: String,
    pub name: String,
    pub block_number: i64,
    pub from: String,
    pub to: String,
    pub cached: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct NFTBidRecord {
    pub buyer_id: String,
    pub price: u64,
    pub coin_id: CoinTokenId
}

pub struct NFTDetail {
    pub desc: NFTDesc,
    pub name: String,
    pub beneficiary: ObjectId,
    pub state: NFTState,
    pub like_count: i64,
    pub block_number: i64,
    pub price: u64,
    pub coin_id: CoinTokenId,
}

pub struct NFTTransRecord {
    pub desc: NFTDesc,
    pub name: String,
    pub block_number: i64,
    pub from: String,
    pub to: String,
    pub nft_cached: Option<String>,
}
