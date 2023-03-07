use crate::data::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct UniObjectBackup {
    noc: NamedObjectCacheRef,
    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
}

impl UniObjectBackup {
    pub fn new(
        noc: NamedObjectCacheRef,
        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
    ) -> Self {
        Self {
            noc,
            data_writer,
            loader,
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        let mut opt = NamedObjectCacheSelectObjectOption::default();
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
        let ret = self.loader.get_object(&object_id).await.map_err(|e| {
            let msg = format!("backup load object failed! id={}, {}", object_id, e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

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
