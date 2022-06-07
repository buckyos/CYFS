pub(crate) mod context;
mod transaction;
mod view;
mod tx_proc;
pub mod tx_executor;

pub use view::{ViewMethodExecutor};
pub use context::*;
pub use transaction::*;


// //tx的执行器（验证器）
// use cyfs_base::*;
// use std::thread::JoinHandle;
// use std::collections::{HashMap, VecDeque};
// use async_std::task;
// use async_std::prelude::*;
// use async_std::stream;
// use std::time::Duration;
// use crate::block::tx::*;

// type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


// struct TxExecutor {
//     pending_tx : HashMap<TxHash,MetaTx>,
// }

// //目前所有Tx共用一个Executor
// impl TxExecutor {
//     pub fn new()->Self {
//         return TxExecutor {
//             pending_tx : HashMap::new(),
//         };
//     }

//     pub fn push_pending_tx(self:&mut TxExecutor,txhash:&TxHash,tx:MetaTx) {
//         unimplemented!();
//     }

//     //这个函数是得到了一个同步过来的block后调用的
//     pub async fn do_tx(tx:&MetaTx) -> Result<()> {
//         unimplemented!();
//     }

//     pub async fn start_packing_tx() {
//         log::info!("tx default executor start...");
//         task::spawn(async move {
//             log::info!("tx default executor start working.");
//             let mut interval = stream::interval(Duration::from_secs(1));
//             let mut working_queue : VecDeque<MetaTx> = VecDeque::new();
//             while let Some(_) = interval.next().await {
//                 //看看working_queue是否为空
//                 if working_queue.len() == 0 {

//                 }

//                 while let Some(tx) = working_queue.pop_front() {
//                     let r = TxExecutor::do_tx(&tx).await;
//                     if let Err(e) = r {
//                         //放入failed queue
//                     } else {
//                         //放入packing queue
//                     }
//                 }
//             }
//         });
//     }
// }



/*
pre or post?
set_block_head() {
    设置快照高度
}

on_sync_block(block) {
    得到正确的db env
    在当前db env里post_do_tx 对tx进行校验
    所有tx in block都校验通过
    set_block_head() //上述修改可以被query_api访问到了
}

on_packing_tx() {
    从commit_queue里读出足够多的tx（并完成排序）
        设置好db env
        按顺序 pre_do_tx，失败的tx记录到failed_list
        成功的tx进入块
    得到一个 owner_block，提交给共识算法确认
    收到足够的确认，修改block_head
}



*/
