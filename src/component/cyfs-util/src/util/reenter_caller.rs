use std::cmp::Eq;
use std::collections::{hash_map::Entry, HashMap};
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

struct ReenterCaller<T>
where
    T: Send + 'static + Clone,
{
    result: Option<T>,
}

impl<T> ReenterCaller<T>
where
    T: Send + 'static + Clone,
{
    pub fn new() -> Self {
        Self { result: None }
    }
}

#[derive(Clone)]
pub struct ReenterCallManager<K, T>
where
    K: Hash + Eq + ToOwned<Owned = K>,
    T: Send + 'static + Clone,
{
    call_list: Arc<Mutex<HashMap<K, Arc<async_std::sync::Mutex<ReenterCaller<T>>>>>>,
}

impl<K, T> ReenterCallManager<K, T>
where
    K: Hash + Eq + ToOwned<Owned = K>,
    T: Send + 'static + Clone,
{
    pub fn new() -> Self {
        Self {
            call_list: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<K, T> ReenterCallManager<K, T>
where
    K: Hash + Eq + ToOwned<Owned = K>,
    T: Send + 'static + Clone,
{
    pub async fn call<F>(&self, key: &K, future: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
    {
        // debug!("will reenter call: key={}", key);

        let caller = {
            let mut list = self.call_list.lock().unwrap();
            match list.entry(key.to_owned()) {
                Entry::Occupied(o) => o.get().clone(),
                Entry::Vacant(v) => {
                    let caller = ReenterCaller::new();
                    let item = Arc::new(async_std::sync::Mutex::new(caller));
                    v.insert(item.clone());
                    item
                }
            }
        };

        // 这里必须使用异步锁，来保证调用中不重入
        let mut item = caller.lock().await;

        // 第一个进来的result一定为空，需要锁住并执行目标闭包
        if item.result.is_none() {
            let ret = future.await;

            // 移除
            {
                let mut list = self.call_list.lock().unwrap();
                list.remove(key);
            }

            // 如果引用计数>1, 说明有重入的操作在等待，需要缓存闭包的返回值
            let ref_count = Arc::strong_count(&caller);
            if ref_count > 1 {
                assert!(item.result.is_none());
                item.result = Some(ret.clone());
            }
            ret
        } else {
            assert!(item.result.is_some());
            item.result.as_ref().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Clone)]
    struct TestReneterCaller {
        caller_manager: ReenterCallManager<String, BuckyResult<u32>>,
        next_value: Arc<AtomicU32>,
    }

    impl TestReneterCaller {
        pub fn new() -> Self {
            Self {
                caller_manager: ReenterCallManager::new(),
                next_value: Arc::new(AtomicU32::new(0)),
            }
        }

        pub async fn call(&self, key: &str) -> BuckyResult<u32> {
            let this = self.clone();
            let owned_key = key.to_owned();
            self.caller_manager
                .call(&key.to_owned(), async move {
                    println!(
                        "will exec call... key={}, next={:?}",
                        owned_key, this.next_value
                    );
                    async_std::task::sleep(std::time::Duration::from_secs(5)).await;
                    println!(
                        "end exec call... key={}, next={:?}",
                        owned_key, this.next_value
                    );

                    let v = this.next_value.fetch_add(1, Ordering::SeqCst);
                    Ok(v)
                })
                .await
        }
    }

    #[async_std::test]
    async fn test_enter_caller_once() {
        let tester = TestReneterCaller::new();
        for i in 0..10 {
            let tester = tester.clone();
            async_std::task::spawn(async move {
                let ret = tester.call("xxxx").await.unwrap();
                assert_eq!(ret, 0);
                println!("caller complete: index={}, ret={}", i, ret);
            });
        }
        async_std::task::sleep(std::time::Duration::from_secs(10)).await;
    }
    #[async_std::test]
    async fn test_enter_caller() {
        let tester = TestReneterCaller::new();
        for i in 0..100 {
            let tester = tester.clone();
            async_std::task::spawn(async move {
                async_std::task::sleep(std::time::Duration::from_secs(i)).await;
                let ret = tester.call("xxxx").await.unwrap();
                println!("caller complete: index={}, ret={}", i, ret);
            });
        }
        async_std::task::sleep(std::time::Duration::from_secs(100)).await;
    }
}
