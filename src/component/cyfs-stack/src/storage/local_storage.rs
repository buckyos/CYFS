use crate::stack::*;
use cyfs_lib::*;

pub struct LocalNOCStorage;

impl LocalNOCStorage {
    pub fn new_noc_collection<T>(id: &str, stack: &CyfsStack) -> NOCCollection<T>
    where
        T: Default + CollectionCodec<T> + Send,
    {
        NOCCollection::new(id, stack.noc_manager().clone_noc())
    }

    pub fn new_noc_collection_sync<T>(id: &str, stack: &CyfsStack) -> NOCCollectionSync<T>
    where
        T: Default + CollectionCodec<T> + Send,
    {
        NOCCollectionSync::new(id, stack.noc_manager().clone_noc())
    }

    pub fn new_noc_collection_rw_sync<T>(id: &str, stack: &CyfsStack) -> NOCCollectionRWSync<T>
    where
        T: Default + CollectionCodec<T> + Send + Sync,
    {
        NOCCollectionRWSync::new(id, stack.noc_manager().clone_noc())
    }
}
