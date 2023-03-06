use super::helper::*;
use crate::archive::ObjectArchiveDecMeta;
use crate::data::BackupDataWriterRef;
use cyfs_base::*;
use cyfs_lib::*;


#[derive(Clone)]
pub struct DecStateBackup {
    isolate_id: ObjectId,
    dec_id: ObjectId,
    dec_root: ObjectId,

    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
    dec_meta: Option<GlobalStateMetaRawProcessorRef>,
}

impl DecStateBackup {
    pub fn new(
        isolate_id: ObjectId,
        dec_id: ObjectId,
        dec_root: ObjectId,
        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
        dec_meta: Option<GlobalStateMetaRawProcessorRef>,
    ) -> Self {
        Self {
            isolate_id,
            dec_id,
            dec_root,

            data_writer,
            loader,
            dec_meta,
        }
    }

    pub async fn run(self) -> BuckyResult<ObjectArchiveDecMeta> {
        let backup_meta =
            ObjectArchiveDecMetaHolder::new(self.dec_id.clone(), self.dec_root.clone());

        let helper = ObjectTraverserHelper::new(
            Some(self.isolate_id.clone()),
            Some(self.dec_id.clone()),
            backup_meta.clone(),
            self.data_writer.clone(),
            self.loader.clone(),
            self.dec_meta.clone(),
        );

        helper.run(&self.dec_root).await?;

        drop(helper);

        Ok(backup_meta.into_inner())
    }
}
