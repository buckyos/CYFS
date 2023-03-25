use crate::backup::BackupStatusManager;
use crate::data::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct UniObjectBackup {
    noc: NamedObjectCacheRef,
    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
    status_manager: BackupStatusManager,
}

impl UniObjectBackup {
    pub fn new(
        noc: NamedObjectCacheRef,
        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
        status_manager: BackupStatusManager,
    ) -> Self {
        Self {
            noc,
            data_writer,
            loader,
            status_manager,
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        let mut opt = NamedObjectCacheSelectObjectOption {
            page_index: 0,
            page_size: 1024,
        };
        
        let filter = NamedObjectCacheSelectObjectFilter::default();

        loop {
            let req = NamedObjectCacheSelectObjectRequest {
                filter: filter.clone(),
                opt: opt.clone(),
            };

            let resp = self.noc.select_object(&req).await?;
            let count = resp.list.len();

            for item in resp.list {
                self.on_object(&item.object_id).await?;
            }

            if count < opt.page_size {
                break;
            }

            opt.page_index += 1;
        }

        Ok(())
    }

    async fn on_object(&self, object_id: &ObjectId) -> BuckyResult<()> {
        self.status_manager.on_object();

        let ret = self.loader.get_object(&object_id).await;
        if ret.is_err() {
            let e = ret.err().unwrap();
            match e.code() {
                BuckyErrorCode::InvalidData | BuckyErrorCode::InvalidFormat | BuckyErrorCode::OutOfLimit => {
                    warn!("backup load object but got error! id={}, {}", object_id, e);
                    self.data_writer.on_error(None, None, object_id, e).await?;

                    return Ok(());
                }
                _ => {
                    let msg = format!("backup load object failed! id={}, {}", object_id, e);
                    error!("{}", msg);
                    return Err(BuckyError::new(e.code(), msg));
                }
            }
        }

        let ret = ret.unwrap();
        if ret.is_none() {
            warn!("backup object missing! root={}", object_id);
            self.data_writer.on_missing(None, None, &object_id).await?;

            return Ok(());
        }

        let data = ret.unwrap();
        self.data_writer
            .add_object(
                &data.object.object_id,
                &data.object.object_raw,
                data.meta.as_ref(),
            )
            .await
    }
}
