use super::com::*;
use super::param::*;
use crate::context::ContextManager;
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

    async fn init_bdt_stack(
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

        if !params.known_device.is_empty() {
            bdt_params.known_device = Some(params.known_device);
        }
        if !params.known_passive_pn.is_empty() {
            bdt_params.passive_pn = Some(params.known_passive_pn);
        }
        bdt_params.outer_cache = Some(device_cache);
        bdt_params.chunk_store = Some(chunk_store);

        bdt_params.ndn_event = ndn_event;

        let ret = Stack::open(params.device, params.secret, bdt_params).await;

        if let Err(e) = ret {
            error!("init bdt stack error: {}", e);
            return Err(e);
        }

        let bdt_stack = ret.unwrap();

        // 等待sn上线
        info!(
            "now will wait for sn online {}......",
            bdt_stack.local_device_id()
        );
        let begin = std::time::Instant::now();
        let ping_clients = bdt_stack.sn_client().ping();
        match ping_clients.wait_online().await {
            Err(e) => {
                error!(
                    "bdt stack wait sn online failed! {}, during={}ms, {}",
                    bdt_stack.local_device_id(),
                    begin.elapsed().as_millis(),
                    e
                );
            }
            Ok(status) => match status {
                SnStatus::Online => {
                    info!(
                        "bdt stack sn online success! {}, during={}ms",
                        bdt_stack.local_device_id(),
                        begin.elapsed().as_millis(),
                    );
                    let bdt_stack = bdt_stack.clone();
                    async_std::task::spawn(async move {
                        let _ = ping_clients.wait_offline().await;
                        retry_sn_list_when_offline(
                            bdt_stack.clone(),
                            ping_clients,
                            std::time::Duration::from_secs(30),
                        );
                    });
                }
                SnStatus::Offline => {
                    error!(
                        "bdt stack wait sn online failed! {}, during={}ms, offline",
                        bdt_stack.local_device_id(),
                        begin.elapsed().as_millis(),
                    );
                    retry_sn_list_when_offline(
                        bdt_stack.clone(),
                        ping_clients,
                        std::time::Duration::from_secs(30),
                    );
                }
            },
        }
        
        Ok(bdt_stack)
    }
}
