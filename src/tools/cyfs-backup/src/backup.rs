use cyfs_base::*;
use cyfs_bdt::{ChunkReaderRef, ChunkReader};
use cyfs_chunk_cache::*;
use cyfs_lib::*;
use cyfs_noc::*;
use cyfs_backup_lib::BackupManager;

use std::sync::Arc;

pub struct BackupService {
    backup_manager: BackupManager,
}

impl BackupService {
    pub async fn new(isolate: &str) -> BuckyResult<Self> {
        let noc = Self::init_noc(isolate).await?;
        let ndc = Self::init_ndc(isolate)?;
        let chunk_manager = Self::init_chunk_manager(isolate).await?;

        let tracker = Self::init_tracker(isolate)?;
        let chunk_reader = Self::create_chunk_reader(chunk_manager, &ndc, tracker);

        let backup_manager = BackupManager::new(noc, ndc, chunk_reader);

        let ret = Self {
            backup_manager,
        };

        Ok(ret)
    }

    pub fn backup_manager(&self) -> &BackupManager {
        &self.backup_manager
    }

    fn init_tracker(isolate: &str) -> BuckyResult<Box<dyn TrackerCache>> {
        use cyfs_tracker_cache::TrackerCacheManager;

        TrackerCacheManager::create_tracker_cache(isolate)
    }

    async fn init_noc(isolate: &str) -> BuckyResult<NamedObjectCacheRef> {
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

    fn init_ndc(isolate: &str) -> BuckyResult<NamedDataCacheRef> {
        use cyfs_ndc::DataCacheManager;

        let ndc = DataCacheManager::create_data_cache(isolate)?;
        Ok(Arc::new(ndc))
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

    fn create_chunk_reader(
        chunk_manager: Arc<ChunkManager>,
        ndc: &Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> ChunkReaderRef {
        use cyfs_bdt_ext::ChunkStoreReader;

        let ret = ChunkStoreReader::new(chunk_manager, (*ndc).clone(), tracker);

        Arc::new(Box::new(ret) as Box<dyn ChunkReader>)
    }
}
