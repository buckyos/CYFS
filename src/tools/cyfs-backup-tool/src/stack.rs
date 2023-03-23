use cyfs_base::*;
use cyfs_bdt::{ChunkReader, ChunkReaderRef};
use cyfs_chunk_cache::*;
use cyfs_lib::*;
use cyfs_noc::*;

use std::sync::Arc;

pub struct StackComponentsHelper {}

impl StackComponentsHelper {
    pub fn init_tracker(isolate: &str) -> BuckyResult<Box<dyn TrackerCache>> {
        use cyfs_tracker_cache::TrackerCacheManager;

        TrackerCacheManager::create_tracker_cache(isolate)
    }

    pub async fn init_noc(isolate: &str) -> BuckyResult<NamedObjectCacheRef> {
        let isolate = isolate.to_owned();

        match NamedObjectCacheManager::create(&isolate).await {
            Ok(noc) => {
                info!("init named object cache manager success!");
                Ok(noc)
            }
            Err(e) => {
                error!("init named object cache manager failed: {}", e);
                Err(e)
            }
        }
    }

    pub fn init_ndc(isolate: &str) -> BuckyResult<NamedDataCacheRef> {
        use cyfs_ndc::DataCacheManager;

        let ndc = DataCacheManager::create_data_cache(isolate)?;
        Ok(Arc::new(ndc))
    }

    pub async fn init_chunk_manager(isolate: &str) -> BuckyResult<ChunkManagerRef> {
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

    pub fn create_chunk_reader(
        chunk_manager: Arc<ChunkManager>,
        ndc: &Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> ChunkReaderRef {
        use cyfs_bdt_ext::ChunkStoreReader;

        let ret = ChunkStoreReader::new(chunk_manager, (*ndc).clone(), tracker);

        Arc::new(Box::new(ret) as Box<dyn ChunkReader>)
    }
}
