use std::sync::Arc;
use crate::{define_obj_type, ListObject, TListObject, ObjTypeCode};
use cyfs_base::*;
use cyfs_lib::{NONAPILevel, NONGetObjectOutputRequest, NONObjectInfo, NONOutputRequestCommon, NONPutObjectOutputRequest, PathOpEnvStub, SharedCyfsStack};

define_obj_type!(ContractStoreChunkList, OBJECT_TYPE_DECAPP_START + 14453);

pub type ContractStoreChunkListObject = ListObject<ContractStoreChunkList, ChunkId>;

#[async_trait::async_trait]
pub trait ContractStoreChunkListManager {
    async fn get_chunk_list(&self, contract_id: &ObjectId) -> BuckyResult<(HashValue, Vec<ChunkId>)>;
    async fn add_chunk_list(&self, contract_id: &ObjectId, mut chunk_list: Vec<ChunkId>) -> BuckyResult<(HashValue, Vec<ChunkId>)>;
    async fn set_chunk_list(&self, contract_id: &ObjectId, chunk_list: Vec<ChunkId>) -> BuckyResult<HashValue>;
}

pub struct CyfsStackContractStoreChunkListManager<'a> {
    stack: Arc<SharedCyfsStack>,
    op_env: &'a PathOpEnvStub,
    path: String,
}

impl<'a> CyfsStackContractStoreChunkListManager<'a> {
    pub fn new(stack: Arc<SharedCyfsStack>, op_env: &'a PathOpEnvStub, path: String) -> Self {
        Self {
            stack,
            op_env,
            path
        }
    }

    async fn get_object_from_noc<T: for<'b> RawDecode<'b>>(&self, object_id: ObjectId) -> BuckyResult<T> {
        self.get_object(None, object_id).await
    }

    async fn put_object_to_noc<T: ObjectType + Sync + Send>(&self, obj: &NamedObjectBase<T>) -> BuckyResult<ObjectId>
        where <T as cyfs_base::ObjectType>::ContentType: cyfs_base::BodyContent + cyfs_base::RawEncode,
              <T as cyfs_base::ObjectType>::DescType: RawEncodeWithContext<cyfs_base::NamedObjectContext> {
        let object_id = obj.desc().calculate_id();
        let object_raw = obj.to_vec()?;
        self.stack.non_service().put_object(NONPutObjectOutputRequest { common: NONOutputRequestCommon {
            req_path: None,
            source: None,
            dec_id: None,
            level: NONAPILevel::NOC,
            target: None,
            flags: 0
        }, object: NONObjectInfo {
            object_id: object_id.clone(),
            object_raw,
            object: None
        },
            access: None
        }).await?;

        Ok(object_id)
    }

    async fn get_object<T: for <'b> RawDecode<'b>>(&self, target: Option<ObjectId>, object_id: ObjectId) -> BuckyResult<T> {
        let mut err = None;
        for _ in 0..3 {
            let resp = match self.stack.non_service().get_object(NONGetObjectOutputRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    source: None,
                    dec_id: None,
                    level: if target.is_none() {NONAPILevel::NOC} else {NONAPILevel::Router},
                    target,
                    flags: 0
                },
                object_id: object_id.clone(),
                inner_path: None
            }).await {
                Ok(resp) => resp,
                Err(e) => {
                    log::error!("get_object {} err {}", object_id.to_string(), e);
                    err = Some(e);
                    continue;
                }
            };

            return T::clone_from_slice(resp.object.object_raw.as_slice());
        }
        Err(err.unwrap())
    }

}

#[async_trait::async_trait]
impl ContractStoreChunkListManager for CyfsStackContractStoreChunkListManager<'_> {
    async fn get_chunk_list(&self, contract_id: &ObjectId) -> BuckyResult<(HashValue, Vec<ChunkId>)> {
        let path = format!("{}/{}/chunk_list", self.path.as_str(), contract_id.to_string());
        let list_id = self.op_env.get_by_path(path.as_str()).await?;
        if list_id.is_none() {
            return Ok((hash_data(Vec::<u8>::new().to_vec()?.as_slice()), Vec::new()));
        }

        let list: ContractStoreChunkListObject = self.get_object_from_noc(list_id.unwrap()).await?;
        Ok((list.list_hash().clone(), list.into_list()))
    }

    async fn add_chunk_list(&self, contract_id: &ObjectId, mut chunk_list: Vec<ChunkId>) -> BuckyResult<(HashValue, Vec<ChunkId>)> {
        let (_, mut cur_list) = self.get_chunk_list(contract_id).await?;
        cur_list.append(&mut chunk_list);
        let list_obj = ContractStoreChunkListObject::new(cur_list);
        let id = self.put_object_to_noc(&list_obj).await?;
        let path = format!("{}/{}/chunk_list", self.path.as_str(), contract_id.to_string());
        self.op_env.set_with_path(path, &id, None, true).await?;
        Ok((list_obj.list_hash().clone(), list_obj.into_list()))
    }

    async fn set_chunk_list(&self, contract_id: &ObjectId, chunk_list: Vec<ChunkId>) -> BuckyResult<HashValue> {
        let list_obj = ContractStoreChunkListObject::new(chunk_list);
        let id = self.put_object_to_noc(&list_obj).await?;
        let path = format!("{}/{}/chunk_list", self.path.as_str(), contract_id.to_string());
        self.op_env.set_with_path(path, &id, None, true).await?;
        Ok(list_obj.list_hash().clone())
    }
}
