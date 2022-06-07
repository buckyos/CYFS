use super::AsyncStorageSync;
use cyfs_base::BuckyError;
use serde::Serialize;

pub struct LocalStore {
    prefix: Option<String>,
    async_storage: AsyncStorageSync,
}

impl LocalStore {
    pub fn new(prefix: Option<&str>, async_storage: AsyncStorageSync) -> Self {
        Self {
            prefix: prefix.map(|v| v.to_owned()),
            async_storage,
        }
    }

    pub fn sub_store(&self, sub_key: &str) -> LocalStore {
        assert!(sub_key.len() > 0);

        let sub_key = match self.prefix.as_ref() {
            Some(v) => format!("{}_{}", v, sub_key),
            None => sub_key.to_owned(),
        };

        Self::new(Some(&sub_key), self.async_storage.clone())
    }

    fn _key(&self, key: &str) -> String {
        match self.prefix.as_ref() {
            Some(v) => format!("{}_{}", v, key),
            None => key.to_owned(),
        }
    }

    pub async fn set(&mut self, key: &str, value: String) -> Result<(), BuckyError> {
        let key = self._key(key);

        let mut async_storage = self.async_storage.lock().await;
        async_storage.set_item(&key, value).await
    }

    pub async fn set_obj<T>(&mut self, key: &str, value: &T) -> Result<(), BuckyError>
    where
        T: ?Sized + Serialize,
    {
        let value = serde_json::to_string(value).unwrap();
        self.set(key, value).await
    }

    pub async fn get(&mut self, key: &str) -> Option<String> {
        let key = self._key(key);

        let async_storage = self.async_storage.lock().await;
        async_storage.get_item(&key).await
    }

    pub async fn get_obj<T>(&mut self, key: &str) -> Result<Option<T>, BuckyError>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.get(key).await {
            Some(v) => match serde_json::from_str(&v) {
                Ok(obj) => Ok(Some(obj)),
                Err(e) => {
                    let msg = format!(
                        "unserialize obj from string error! err={}, content={}",
                        e, v
                    );
                    error!("{}", msg);

                    Err(BuckyError::from(msg))
                }
            },
            None => Ok(None),
        }
    }

    pub async fn remove(&mut self, key: &str) -> Option<()> {
        let key = self._key(key);

        let mut async_storage = self.async_storage.lock().await;
        async_storage.remove_item(&key).await
    }

    pub async fn remove_obj<T>(&mut self, key: &str) -> Result<Option<()>, BuckyError>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.remove(key).await {
            // Some(v) => match serde_json::from_str(&v) {
            //     Ok(obj) => Ok(Some(obj)),
            //     Err(e) => {
            //         let msg = format!(
            //             "unserialize obj from string error! err={}, content={}",
            //             e, v
            //         );
            //         error!("{}", msg);

            //         Err(BuckyError::from(msg))
            //     }
            // },
            Some(_v) => Ok(Some(())),
            None => Ok(None),
        }
    }

    pub async fn clear(&mut self) {
        match self.prefix.as_ref() {
            Some(value) => {
                let prefix = format!("{}_", value);
                let mut async_storage = self.async_storage.lock().await;
                async_storage.clear_with_prefix(&prefix).await;
            }
            None => {
                let mut async_storage = self.async_storage.lock().await;
                async_storage.clear().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::super::{into_async_storage_sync, AsyncStorage, FileStorage, SqliteStorage};
    use super::LocalStore;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::sync::{Arc, Mutex};
    extern crate simple_logger;
    #[derive(Serialize, Deserialize, Eq, PartialEq, Debug)]
    struct TestObj {
        name: String,
        value: i32,
    }

    #[async_std::test]
    async fn test_file_store() {
        let mut file_storage = FileStorage::new();
        file_storage.init("test").await.unwrap();

        let mut store = LocalStore::new(None, into_async_storage_sync(file_storage));
        store.set("name", "xxx".to_owned()).await.unwrap();

        assert_eq!(store.get("name").await.unwrap(), "xxx");

        let mut store = store.sub_store("app");
        store.set("name", "xxx".to_owned()).await.unwrap();

        assert_eq!(store.get("name").await.unwrap(), "xxx");

        let test_obj = TestObj {
            name: "jack".to_owned(),
            value: -1,
        };

        store.set_obj("obj", &test_obj).await.unwrap();

        assert_eq!(
            store.get_obj::<TestObj>("obj").await.unwrap().unwrap(),
            test_obj
        );

        assert_eq!(
            store.remove_obj::<TestObj>("obj").await.unwrap().unwrap(),
            ()
        );
        assert_eq!(store.remove_obj::<TestObj>("obj").await.unwrap(), None);

        store.clear().await;

        assert!(store.get("obj").await == None);
        assert!(store.get_obj::<TestObj>("obj").await.unwrap() == None);

        store.set_obj("obj", &test_obj).await.unwrap();
    }

    #[async_std::test]
    async fn test_sqlite_store() {
        let mut sqlite_storage = SqliteStorage::new();
        sqlite_storage.init("test").await.unwrap();

        let mut store = LocalStore::new(None, into_async_storage_sync(sqlite_storage));
        store.set("name", "xxx".to_owned()).await.unwrap();

        assert_eq!(store.get("name").await.unwrap(), "xxx");

        let mut store = store.sub_store("app");
        store.set("name", "xxx".to_owned()).await.unwrap();

        assert_eq!(store.get("name").await.unwrap(), "xxx");

        let test_obj = TestObj {
            name: "jack".to_owned(),
            value: -1,
        };

        store.set_obj("obj", &test_obj).await.unwrap();

        assert_eq!(store.remove_obj::<TestObj>("obj").await.unwrap(), Some(()));
        assert_eq!(store.remove_obj::<TestObj>("obj").await.unwrap(), None);

        store.clear().await;

        assert!(store.get("obj").await == None);
        assert!(store.get_obj::<TestObj>("obj").await.unwrap() == None);

        store.set_obj("obj", &test_obj).await.unwrap();
    }
}
