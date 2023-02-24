use cyfs_base::DeviceId;
use super::collection::*;
use super::remote_noc::*;
use crate::stack::*;

pub struct RemoteNOCStorage;

impl RemoteNOCStorage {
    pub fn new_noc_collection<T>(id: &str, stack: &SharedCyfsStack) -> NOCCollection<T>
    where
        T: Default + CollectionCodec<T> + Send + 'static,
    {
        let remote_noc = RemoteNamedObjectCache::new(
            stack.non_service().clone_processor(),
            &stack.local_device_id(),
        );
        NOCCollection::new(id, remote_noc.into_noc())
    }

    pub fn new_noc_collection_sync<T>(id: &str, stack: &SharedCyfsStack) -> NOCCollectionSync<T>
    where
        T: Default + CollectionCodec<T> + Send + 'static,
    {
        let remote_noc = RemoteNamedObjectCache::new(
            stack.non_service().clone_processor(),
            &stack.local_device_id(),
        );
        NOCCollectionSync::new(id, remote_noc.into_noc())
    }

    pub fn new_noc_collection_sync_uni<T>(id: &str, stack: &UniCyfsStackRef, device_id: &DeviceId) -> NOCCollectionSync<T>
        where
            T: Default + CollectionCodec<T> + Send + 'static,
    {
        let remote_noc = RemoteNamedObjectCache::new(
            stack.non_service().clone(),
            device_id,
        );
        NOCCollectionSync::new(id, remote_noc.into_noc())
    }

    pub fn new_noc_collection_rw_sync<T>(
        id: &str,
        stack: &SharedCyfsStack,
    ) -> NOCCollectionRWSync<T>
    where
        T: Default + CollectionCodec<T> + Send + Sync + 'static,
    {
        let remote_noc = RemoteNamedObjectCache::new(
            stack.non_service().clone_processor(),
            &stack.local_device_id(),
        );
        NOCCollectionRWSync::new(id, remote_noc.into_noc())
    }
}
