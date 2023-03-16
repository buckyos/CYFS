use super::com::*;
use super::param::*;
use crate::context::ContextManager;
use crate::sn::BdtStackSNHelper;

use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_chunk_cache::{ChunkManager, ChunkManagerRef};
use cyfs_lib::*;
use std::sync::Arc;

pub struct BdtStackHelper;

impl BdtStackHelper {
    pub async fn init_named_data_components(
        isolate: &str,
        noc: NamedObjectCacheRef,
        device_manager: Box<dyn DeviceCache>,
    ) -> BuckyResult<NamedDataComponents> {
        // 初始化data cache和tracker
        let ndc = Self::init_ndc(isolate)?;
        let tracker = Self::init_tracker(isolate)?;

        let chunk_manager = Self::init_chunk_manager(isolate).await?;

        let context_manager = ContextManager::new(noc.clone(), device_manager);

        let named_data_components =
            NamedDataComponents::new(chunk_manager, ndc, tracker, context_manager.clone());

        Ok(named_data_components)
    }

    fn init_ndc(isolate: &str) -> BuckyResult<Box<dyn NamedDataCache>> {
        use cyfs_ndc::DataCacheManager;

        DataCacheManager::create_data_cache(isolate)
    }

    fn init_tracker(isolate: &str) -> BuckyResult<Box<dyn TrackerCache>> {
        use cyfs_tracker_cache::TrackerCacheManager;

        TrackerCacheManager::create_tracker_cache(isolate)
    }

    async fn init_chunk_manager(isolate: &str) -> BuckyResult<ChunkManagerRef> {
        let chunk_manager = Arc::new(ChunkManager::new());
        match chunk_manager.init(isolate).await {
            Ok(()) => {
                info!("init chunk manager success!");
                Ok(chunk_manager)
            }
            Err(e) => {
                info!("init chunk manager failed!.{}", &e);
                Err(e)
            }
        }
    }

    pub async fn init_bdt_stack(
        params: BdtStackParams,
        device_cache: Box<dyn DeviceCache>,
        isolate: &str,
        named_data_components: &NamedDataComponents,
        ndn_event: Option<Box<dyn NdnEventHandler>>,
    ) -> BuckyResult<StackGuard> {
        let chunk_store = named_data_components.new_chunk_reader();

        let mut bdt_params = StackOpenParams::new(isolate);

        if !params.tcp_port_mapping.is_empty() {
            bdt_params.tcp_port_mapping = Some(params.tcp_port_mapping);
        }

        if let Some(sn_only) = params.udp_sn_only {
            bdt_params.config.interface.udp.sn_only = sn_only;
        }

        // select sn_list via the sn_mode config
        let wait_online;
        let sn_list = match params.sn_mode {
            SNMode::Normal => {
                wait_online = true;
                Some(params.known_sn.clone())
            }
            SNMode::None => {
                wait_online = false;
                warn!("sn_mode is none, now will clear the sn_list!");
                None
            }
        };

        bdt_params.known_sn = Some(params.known_sn);

        if !params.known_device.is_empty() {
            bdt_params.known_device = Some(params.known_device);
        }
        if !params.known_passive_pn.is_empty() {
            bdt_params.passive_pn = Some(params.known_passive_pn);
        }
        bdt_params.outer_cache = Some(device_cache);
        bdt_params.chunk_store = Some(chunk_store);

        bdt_params.ndn_event = ndn_event;

        let has_wan_endpoint = params.device.has_wan_endpoint();
        let ret = Stack::open(params.device, params.secret, bdt_params).await;

        if let Err(e) = ret {
            error!("init bdt stack error: {}", e);
            return Err(e);
        }

        let bdt_stack = ret.unwrap();

        // apply the sn list
        let ping_clients = if let Some(sn_list) = sn_list {
            bdt_stack.sn_client().reset_sn_list(sn_list)
        } else {
            bdt_stack.sn_client().ping()
        };

        if wait_online {
            if has_wan_endpoint {
                warn!("current device has wan endpoint, now will use async wait-online");
                let bdt_stack = bdt_stack.clone();
                async_std::task::spawn(async move {
                    let _ = BdtStackSNHelper::wait_sn_online(&bdt_stack, ping_clients).await;
                });
            } else {
                BdtStackSNHelper::wait_sn_online(&bdt_stack, ping_clients).await?;
            }
        }

        Ok(bdt_stack)
    }
}
