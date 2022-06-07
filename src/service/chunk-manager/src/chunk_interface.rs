use std::path::PathBuf;
use tide::{Request};

use cyfs_base::*;

use crate::chunk_processor;
use crate::chunk_delegate;
use crate::chunk_tx;
use crate::chunk_meta::CHUNK_META;
use crate::chunk_context::ChunkContext;

pub struct ChunkInterface {
    database: PathBuf,
    desc: String,
    ctx: Option<ChunkContext>,
}

impl ChunkInterface {
    pub fn new(database_: &PathBuf)->ChunkInterface{
        ChunkInterface {
            database: database_.clone(),
            desc: "device".to_owned(),
            ctx: None,
        }
    }

    pub async fn init(&mut self, chunk_dir: &PathBuf)-> BuckyResult<()> {
        info!("@cache miner init device desc");
        assert!(self.ctx.is_none());

        let (device, pri_key) = cyfs_util::get_device_desc(self.desc.as_str())?;
        
        let device_id = device.desc().device_id();

        self.ctx = Some(ChunkContext{
            chunk_dir: chunk_dir.clone(),
            device_id,
            device,
            pri_key
        });

        info!("@init database");
        {
            let mut create_table_list:Vec<String> = Vec::new();
            chunk_delegate::init_table(&mut create_table_list);
            chunk_tx::init_table(&mut create_table_list);
            let mut chunk_meta = CHUNK_META.lock().await;
            chunk_meta.init(&self.database, &create_table_list).map_err(|e|{
                error!("init chunk meta failed, err:{}", e);
                std::io::Error::from(std::io::ErrorKind::Interrupted)
            })?;
        }

        Ok(())
    }

    pub async fn run(&self) -> Result<(), std::io::Error>  {
        info!("@init app");
        let mut app = tide::new();

        let ctx = self.ctx.as_ref().unwrap().clone();
        app.at(cyfs_chunk::method_path::GET_CHUNK).post(move |mut req: Request<()>| {
            let ctx = ctx.clone();
            async move { 
                chunk_processor::get_chunk(ctx, &mut req).await.map_err(|e|{
                    tide::Error::from_str(tide::StatusCode::ServiceUnavailable, e.to_string())
                })
            }
        });

        let ctx = self.ctx.as_ref().unwrap().clone();
        app.at(cyfs_chunk::method_path::SET_CHUNK).post(move |mut req: Request<()>| {
            let ctx = ctx.clone();
            async move {
                chunk_processor::set_chunk(ctx, & mut req).await.map_err(|e|{
                    tide::Error::from_str(tide::StatusCode::ServiceUnavailable, e.to_string())
                })
            }
        });

        let ctx = self.ctx.as_ref().unwrap().clone();
        app.at(cyfs_chunk::method_path::CREATE_CHUNK_DELEGATE).post(move |mut req: Request<()>| {
            let ctx = ctx.clone();
            async move {
                chunk_processor::create_chunk_delegate(ctx, & mut req).await.map_err(|e|{
                    tide::Error::from_str(tide::StatusCode::ServiceUnavailable, e.to_string())
                })
            }
        });

        let ctx = self.ctx.as_ref().unwrap().clone();
        app.at(cyfs_chunk::method_path::REDEEM_CHUNK_PROOF).post(move |mut req: Request<()>| {
            let ctx = ctx.clone();
            async move {
                chunk_processor::redeem_chunk_proof(ctx, & mut req).await.map_err(|e|{
                    tide::Error::from_str(tide::StatusCode::ServiceUnavailable, e.to_string())
                })
            }
        });

        let ctx = self.ctx.as_ref().unwrap().clone();
        app.at(cyfs_chunk::method_path::QUERY_CHUNK_DELEGATE).post(move |mut req: Request<()>| {
            let ctx = ctx.clone();
            async move {
                chunk_processor::query_chunk_delegate(ctx, & mut req).await.map_err(|e|{
                    tide::Error::from_str(tide::StatusCode::ServiceUnavailable, e.to_string())
                })
            }
        });

        info!("start daemon");
        {
            // let ctx = self.ctx.as_ref().unwrap().clone();
            // std::thread::spawn( move || {
            //     async_std::t::block_on(async {
            //         println!("run daemon...");
            //         let ctx = ctx.clone();
            //         let mut daemon = CHUNK_DAEMON.lock().await;
            //         println!("run daemon...");
            //         let ret = daemon.run(ctx).await;
            //         if let Err(e) = ret {
            //             error!("cache_daemon run err:{}", e);
            //         }
            //     });
            // });
        }

        let addr = format!("127.0.0.1:{}", ::cyfs_base::CHUNK_MANAGER_PORT);
        app.listen(addr).await?;

        Ok(())
    }
}
