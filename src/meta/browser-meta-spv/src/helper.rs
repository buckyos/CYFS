use cyfs_base_meta::MetaTxBody;

pub fn parse_tx(tx: &MetaTxBody) -> (u32, String) {
    let mut to = "".to_string();
    let tx_type = match tx {
        MetaTxBody::TransBalance(tx) => {
            to = tx.to[0].0.to_string();
            0
        }
        MetaTxBody::CreateUnion(_) => {1}
        MetaTxBody::DeviateUnion(tx) => {
            to = tx.body.union.to_string();
            2
        }
        MetaTxBody::WithdrawFromUnion(tx) => {
            to = tx.union.to_string();
            3
        }
        MetaTxBody::CreateDesc(_) => {4}
        MetaTxBody::UpdateDesc(_) => {5}
        MetaTxBody::RemoveDesc(tx) => {
            to = tx.id.to_string();
            6
        }
        MetaTxBody::BidName(tx) => {
            to = tx.name.clone();
            7
        }
        MetaTxBody::UpdateName(tx) => {
            to = tx.name.clone();
            8
        }
        MetaTxBody::TransName(tx) => {
            to = tx.new_owner.to_string();
            9
        }
        MetaTxBody::Contract(_) => {10}
        MetaTxBody::SetConfig(_) => {11}
        MetaTxBody::AuctionName(tx) => {
            to = tx.name.clone();
            12
        }
        MetaTxBody::CancelAuctionName(tx) => {
            to = tx.name.clone();
            13
        }
        MetaTxBody::BuyBackName(tx) => {
            to = tx.name.clone();
            14
        }
        MetaTxBody::BTCCoinageRecord(_) => {15}
        MetaTxBody::WithdrawToOwner(tx) => {
            to = tx.id.to_string();
            16
        }
        MetaTxBody::CreateMinerGroup(_) => {17}
        MetaTxBody::UpdateMinerGroup(_) => {18}
        MetaTxBody::CreateSubChainAccount(_) => {19}
        MetaTxBody::UpdateSubChainAccount(_) => {20}
        MetaTxBody::SubChainWithdraw(_) => {21}
        MetaTxBody::WithdrawFromSubChain(_) => {22}
        MetaTxBody::SubChainCoinageRecord(_) => {23}
        MetaTxBody::Extension(_) => {24}
        MetaTxBody::CreateContract(_) => {25}
        MetaTxBody::CreateContract2(_) => {26}
        MetaTxBody::CallContract(tx) => {
            to = tx.address.to_string();
            27
        }
        MetaTxBody::SetBenefi(_) => {28}
        MetaTxBody::NFTCreate(_) => {29}
        MetaTxBody::NFTAuction(tx) => {
            to = tx.nft_id.to_string();
            30
        }
        MetaTxBody::NFTBid(tx) => {to = tx.nft_id.to_string();31}
        MetaTxBody::NFTBuy(tx) => {to = tx.nft_id.to_string();32}
        MetaTxBody::NFTSell(tx) => {to = tx.nft_id.to_string();33}
        MetaTxBody::NFTApplyBuy(tx) => {to = tx.nft_id.to_string();34}
        MetaTxBody::NFTCancelApplyBuyTx(tx) => {to = tx.nft_id.to_string();35}
        MetaTxBody::NFTAgreeApply(tx) => {to = tx.nft_id.to_string();36}
        MetaTxBody::NFTLike(tx) => {to = tx.nft_id.to_string();37}
        MetaTxBody::NFTCancelSellTx(tx) => {
            to = tx.nft_id.to_string();
            38
        }
        MetaTxBody::NFTSetNameTx(tx) => {
            to = tx.nft_id.to_string();
            39
        }
        MetaTxBody::NFTCreate2(_) => {
            40
        }
        MetaTxBody::NFTSell2(tx) => {
            to = tx.nft_id.to_string();
            41
        }
    };

    (tx_type, to)
}