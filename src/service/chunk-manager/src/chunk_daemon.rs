use std::time::Instant;
use lazy_static::lazy_static;
use async_std::sync::{Mutex};

use cyfs_base::{BuckyResult, BuckyError};

use crate::chunk_manager::ChunkManager;
use crate::chunk_context::ChunkContext;
use crate::chunk_delegate::{fetch_init_chunk_delegates, open_chunk_delegate};

pub struct ChunkDaemon {

}

impl ChunkDaemon {
    // add code here
    pub fn new() -> ChunkDaemon {

        ChunkDaemon {

        }
    }

    pub async fn run(&mut self, ctx: ChunkContext) -> Result<(), BuckyError> {
        let _ = self.run_check_loop(ctx).await;
        Ok(())
    }

    async fn run_check_loop(&mut self, ctx: ChunkContext)->BuckyResult<()> {

        let chunk_manager = ChunkManager::new(&ctx);

        
        
        let timer = timer::Timer::new();
        let (tx, rx) = std::sync::mpsc::channel();

        let _guard = timer.schedule_repeating(chrono::Duration::seconds(10), move || {
            // This closure is executed on the scheduler thread,
            // so we want to move it away asap.

            let ret = tx.send(());
            if let Err(e) = ret {
                error!("send timer notify error! err={}", e);
            }
        });

        let mut last_redeem_state = Instant::now();

        info!("@run chunk daemon loop");
        loop {
            let ret = rx.recv();
            if let Err(e) = ret {
                error!("recv timer notify error! err={}", e);
                break;
            }

            if last_redeem_state.elapsed().as_secs() > 10 {
                info!("@will check and delegate...");
                let _ = Self::delegate(&ctx, &chunk_manager).await;

                last_redeem_state = Instant::now();
            }
        }

        Ok(())
    }

    async fn delegate(_ctx: & ChunkContext, chunk_manager: &ChunkManager)->BuckyResult<()>{

        let list = fetch_init_chunk_delegates(10).await?;

        info!("@ un delegate chunk count:{}", list.len());

        for item in list {
            let _ = async_std::io::timeout(std::time::Duration::from_secs(15), async {

                info!("@ delegate chunk: {:?} to miner: {:?} ...", item.chunk_id, item.miner_device_id);

                let _ = chunk_manager.delegate(
                    &item.miner_device_id, 
                    &item.chunk_id, 
                    &item.price
                ).await.map_err(|e|{
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                })?;

                info!("@ delegate chunk: {:?} to miner: {:?} success, now open it.", item.chunk_id, item.miner_device_id);

                let _ = open_chunk_delegate(&item.miner_device_id, &item.chunk_id).await.map_err(|e|{
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                })?;
    
                Ok(())
            }).await.map_err(|e|{
                BuckyError::from(e)
            });
        }
        
        Ok(())
    }
}

lazy_static! {
    pub static ref CHUNK_DAEMON: Mutex<ChunkDaemon> = {
        return Mutex::new(ChunkDaemon::new());
    };
}