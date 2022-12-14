extern crate core;

pub use chain::{Chain, Miner};
pub use creator::*;
pub use cyfs_base_meta::*;
pub use db_helper::*;
pub use executor::*;
pub use extension::*;
pub use helper::*;
pub use network::*;
pub use server::*;
pub use state_storage::*;
pub use nft_auction::*;

mod tmp_manager;
mod state_storage;
mod executor;
mod chain;
mod creator;
mod name_auction;
mod events;
mod rent;
mod stat;
#[macro_use]
mod helper;
mod mint;
mod network;
mod db_helper;
mod extension;
mod server;
mod meta_backend;
mod nft_auction;

/*
尝试列出所有的TX
# 超级节点维护和共识算法相关
## 合约操作（我们提供内置类型而不是全合约化的目的，就是）
EnableContract(code_hash:u256,)
## 创建抵押地址

## 和当地法律配合
可以通过执法机构依据法律无效化某个TX
也可以要求，某类TX生效的前置条件是获得法律的批准
在不同的国家可以建立分级的法律机构



# 抵押操作 //CoinBase操作，无需手工触发
每个Block的起始:
CoinedBFC (peerid,coin_num,btc_tx_hash,btc_block_height)
OnNewBlock()
{
//btc_tx_hash指向的是一个被确认的（12个块以前），转入了抵押地址的交易交易，交易的附言里有peerid
    if btc_block_height = btc_head { //btc head比正常的btc要慢12个块
        for tx in btc_client.head() {
            if tx.to in SuperNode.bank_address {
                if is_valid_peerid(tx.tips) {
                    CoinedBFC(tx.tips,tx.out/SuperNode.bfcPrices,tx.hash
                }
            }
        }
    }
}

# 基础数字货币操作
TransCoinTx(seq:u32,from:Peerid,to:vec<Peerid>,cointype:u8,value:vec<u64>) {
    let v = AccountDB.get_account_value(from,cointype)
    if v > value.sum {
        if is_contact(from) {
            let c =ContactDB.get(from)
            if c {
                if !c.can_trans(from,to,cointype,value) {
                    return;
                }
            }
        } else {
            AccountDB.set_account_value(from,conintype,v-value.sum);
            for p in value {
                if is_contact(p) {

                }
            }
        }
    }
}
//token_contact.trans(seq:u32,from:Peerid,to:vec<Peerid>,value:vec<u64>)
TransTokenTx(seq:u32,from:Peerid,to:vec<Peerid>,token_contact_id:u256,value:vec<u64>) （语法糖，相当于） {
    let token_contact = ContactDB.get(token_contact_id)
    token_contact.trans() //这个contact必须是个token contact

}

//创立Coin<->Token兑换合约


# id-desc
//名字服务特殊处理（价格也更贵）
CreateNameDescTx(owner:Peerid,name:string,desc:string,cointype:u8,price:u64,value:u64) {

}

UpdateNameDescTx(name:string,desc:string) {

}
//一些复杂的名字服务的转移功能，需要使用智能合同来扩充
比如抢注，拍卖等等
很多id都有拍卖的价值，为了简化逻辑，我们的功能为Group（公司）可以被公开出售

//其它的id-desc
CreateDescTx(owner:Peerid,id:Peerid,desc:string) {

}

UpdateDescTx(opid:Peerid,key:Peerid,desc:string) {

}

//在区块链上删除一个东西并不会保证实时性
RemoveDescTx(opid:Peerid,key:Peerid) {
}

//id余额与租金扣除
id的owner租用meta-chain的空间来保存desc,转入id的余额能转出么？支持自动扣费么？
在meta-chain上保存desc,会根据保存的时间、desc的大小、desc的租用单价进行定期扣款，直到id上没有余额
扣除租金的工作类似CoinBase,从节约TX历史记录的角度来看，是可以做到不存储扣除TX的。


//
#org管理(公司法)
“但一个资源被多个人拥有时，那么这个资源的owner是一个group”


# 版权相关TX


*/

// fn main() {
//     //parse start params
//     let a:u32 = ObjectType::Device as u32;

//     init_log();
//     log::warn!("ffs meta node start... {}",a);
//     //parse config

//     //以sync模式初始化网络模块（使用其他peerid)，与其它meta-node建立连接并获得block高度和block head信息
//     // (TODO:网络模块Gateway化，miner只关心gateway提供的Block,TX的Sub与Pub)
//     //判断本地block是否存在分叉，选择版本树

//     //选定版本树，确定head-block
//     //初始化区块存储引擎，并等待加载完成
//     //初始化智能合同引擎，加载所有的已知智能合同代码（TODO：根据场景优先编写智能合同代码）
//     //加载最近的状态快照，校验快照的合法性 （TODO：用什么技术来保存状态快照？这个技术选型和智能合同的功能的能力有关）
//     //基础状态：
//     //  超级节点和共识算法依赖的信息
//     //  非合约账户的指定币种(Coin)的余额,官方支持的币种不会太多
//     //  id租约（不提供查询只需要有desc长度和owner信息，用来支持扣租金和处理update即可）
//     //      挑战：在OOD上保存所有的id-desc(平均100万条需要2G存储空间，2T的SSD大概能存储10亿条记录)
//     //      经济角度：1KB的desc的日存储价格是1分的话，维护一个域名的成本是3.65元/年
//     //              一个文件或文件夹的desc日存储价格是1kb 0.1分/天？ 一年3毛？
//     //  状态缓存存在的目的是提高TX的验证性能
//     //  可以针对不同的TX创建不同的专用缓存，进行极限优化。=>TODO:详细的设计TX的类型非常重要 ，不同类型的TX都可以并行执行？
//     //同步状态到 head-block，同步的过程中会更新状态快照

//     //以super node的身份启动block-sync模块 （加入共识算法group）
//     //以super node的身份开始接收并打包tx

//     //开启状态查询API接口


//     log::warn!("ffs meta node end.");
// }
