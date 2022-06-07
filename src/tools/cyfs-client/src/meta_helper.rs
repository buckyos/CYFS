use cyfs_meta_lib::MetaClient;

use log::*;
use cyfs_base::*;
use cyfs_base_meta::*;

pub async fn create_file_desc_sync(meta_client: &MetaClient, caller: &StandardObject, secret: &PrivateKey, desc: &File)->BuckyResult<()> {
    let id = desc.desc().calculate_id();

    if meta_client.get_desc(&id).await.is_ok() {
        info!("file {} desc already on meta, success", id);
        return Ok(());
    } else {
        let hash = meta_client.create_desc(caller, &SavedMetaObject::File(desc.clone()), 0, 0, 0, secret).await?;
        info!("put file {} desc to meta, hash {}", id, &hash);
        Ok(())
    }
}

pub async fn create_desc(meta_client: &MetaClient, caller: &StandardObject, secret: &PrivateKey, desc: AnyNamedObject) -> BuckyResult<()> {
    let id = desc.calculate_id();

    if meta_client.get_desc(&id).await.is_ok() {
        info!("desc {} already on meta, success", id);
        return Ok(());
    } else {
        let hash = meta_client.create_desc(caller, &SavedMetaObject::Data(Data { id, data: desc.to_vec()? }), 0, 0, 0, secret).await?;
        info!("put desc {} to meta, hash {}", id, &hash);
        Ok(())
    }
}
