use super::helper::*;
use crate::data::*;
use crate::meta::ObjectArchiveDataSeriesMeta;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

struct RootObjectCategory {
    object_type: u16,
    ref_depth: u32,
}

struct RootObjectCategoryManager {
    list: Vec<RootObjectCategory>,
}

impl RootObjectCategoryManager {
    pub fn new() -> Self {
        let mut list = vec![];

        let item = RootObjectCategory {
            object_type: CoreObjectType::Storage.as_u16(),
            ref_depth: 0,
        };
        list.push(item);

        let item = RootObjectCategory {
            object_type: CoreObjectType::TransContext.as_u16(),
            ref_depth: 0,
        };
        list.push(item);

        let item = RootObjectCategory {
            object_type: ObjectTypeCode::People.to_u16(),
            ref_depth: 0,
        };
        list.push(item);

        let item = RootObjectCategory {
            object_type: ObjectTypeCode::SimpleGroup.to_u16(),
            ref_depth: 0,
        };
        list.push(item);

        let item = RootObjectCategory {
            object_type: ObjectTypeCode::Device.to_u16(),
            ref_depth: 0,
        };
        list.push(item);

        Self { list }
    }
}

pub struct RootObjectBackup {
    roots: RootObjectCategoryManager,
    noc: NamedObjectCacheRef,

    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
}

impl RootObjectBackup {
    pub fn new(
        noc: NamedObjectCacheRef,

        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
    ) -> Self {
        Self {
            roots: RootObjectCategoryManager::new(),
            noc,
            data_writer,
            loader,
        }
    }

    pub async fn run(&self) -> BuckyResult<ObjectArchiveDataSeriesMeta> {
        let dec_id = ObjectId::default();
        let dec_root = ObjectId::default();

        let backup_meta = ObjectArchiveDecMetaHolder::new(dec_id, dec_root);

        let helper = ObjectTraverserHelper::new(
            None,
            None,
            backup_meta.clone(),
            self.data_writer.clone(),
            self.loader.clone(),
            None,
        );

        let mut opt = NamedObjectCacheSelectObjectOption::default();

        for category in &self.roots.list {
            loop {
                let req = NamedObjectCacheSelectObjectRequest {
                    filter: NamedObjectCacheSelectObjectFilter {
                        obj_type: Some(category.object_type),
                    },
                    opt: opt.clone(),
                };

                let resp = self.noc.select_object(&req).await?;
                let count = resp.list.len();

                for item in resp.list {
                    helper.run(&item.object_id).await?;
                }

                if count < opt.page_size {
                    break;
                }

                opt.page_index += 1;
            }
        }

        drop(helper);

        let dec_meta = backup_meta.into_inner();

        Ok(dec_meta.meta)
    }
}
