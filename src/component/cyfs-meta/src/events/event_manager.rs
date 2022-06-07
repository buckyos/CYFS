use crate::state_storage::{StateRef, StateWeakRef};
use crate::{BlockDesc, State, MetaExtensionManager, get_meta_err_code};
use crate::executor::context::{ConfigRef, ConfigWeakRef};
use cyfs_base_meta::*;
use std::collections::HashMap;
use crate::helper::{ArcWeakHelper};
use cyfs_base::*;
use std::future::Future;
use std::sync::{Arc, Weak, Mutex};
use async_trait::async_trait;

#[async_trait]
pub trait EventListener: 'static + Send + Sync {
    async fn call(&self, block_desc: BlockDesc, event: Event) -> BuckyResult<EventResult>;
}

#[async_trait]
impl<F, Fut> EventListener for F
    where
        F: Send + Sync + 'static + Fn(BlockDesc, Event) -> Fut,
        Fut: Future<Output=BuckyResult<EventResult>> + 'static + Send
{
    async fn call(&self, block_desc: BlockDesc, event: Event) -> BuckyResult<EventResult> {
        let fut = (self)(block_desc, event);
        fut.await
    }
}

pub type EventManagerRef = Arc<EventManager>;
pub type EventManagerWeakRef = Weak<EventManager>;

pub struct EventManager {
    ref_state: StateWeakRef,
    config: ConfigWeakRef,
    listener_hash: Mutex<HashMap<EventType, Arc<dyn EventListener>>>,
}

impl EventManager {
    pub fn new(state: &StateRef, config: &ConfigRef) -> EventManagerRef {
        let manager = EventManagerRef::new(EventManager {
            ref_state: StateRef::downgrade(state),
            config: ConfigRef::downgrade(config),
            listener_hash: Mutex::new(HashMap::new())
        });

        let state = state.clone();
        manager.register_listener(EventType::Extension, move |block_desc: BlockDesc, event: Event| {
            let ref_state = state.clone();
            async move {
                if let Event::Extension(extension_event) = &event {
                    let ret= MetaExtensionManager::get_extension(&extension_event.extension_type);
                    if ret.is_some() {
                        ret.unwrap().on_event(&ref_state, &block_desc, extension_event).await
                    } else {
                        Err(crate::meta_err!(ERROR_SKIP))
                    }
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            }
        });

        manager
    }

    pub async fn add_or_update_cycle_event(&self, key: &str, event: &Event, cycle: i64, init_height: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.add_or_update_cycle_event(key, event, cycle, init_height).await
    }

    pub async fn get_cycle_event(&self, key: &str, cycle: i64) -> BuckyResult<Event> {
        self.ref_state.to_rc()?.get_cycle_event_by_key(key, cycle).await
    }

    pub async fn get_cycle_event2(&self, key: &str, cycle: i64) -> BuckyResult<(i64, Event)> {
        self.ref_state.to_rc()?.get_cycle_event_by_key2(key, cycle).await
    }

    pub async fn drop_cycle_event(&self, key: &str, cycle: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.drop_cycle_event(key, cycle).await
    }

    pub async fn add_or_update_once_event(&self, key: &str, event: &Event, height: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.add_or_update_once_event(key, event, height).await
    }

    pub async fn get_once_event(&self, key: &str) -> BuckyResult<Event> {
        self.ref_state.to_rc()?.get_once_event_by_key(key).await
    }

    pub async fn drop_once_event(&self, key: &str) -> BuckyResult<()> {
        self.ref_state.to_rc()?.drop_once_event_by_key(key).await
    }

    pub fn register_listener(&self, event_type: EventType, listener: impl EventListener) {
        let mut map = self.listener_hash.lock().unwrap();
        map.insert(event_type, Arc::new(listener));
    }

    pub async fn change_event_cycle(&self, cur_block: &BlockDesc, event_list: &Vec<EventType>, from: i64, to: i64) -> BuckyResult<()> {
        if from == to {
            return Ok(());
        }
        self.ref_state.to_rc()?.create_cycle_event_table(to).await?;
        let events = self.ref_state.to_rc()?.get_all_cycle_events(from).await?;
        self.ref_state.to_rc()?.drop_all_cycle_events(from).await?;
        for (real_key, start_height, event) in events {
            let event_type = event.get_type();
            let mut find = false;
            for t in event_list {
                if event_type == *t {
                    find = true;
                    break;
                }
            }
            if !find {
                continue;
            }
            let offset = start_height % from;
            let latest_trigger_block = cur_block.number() - cur_block.number() % from + offset - from;
            if latest_trigger_block < 0 {
                self.ref_state.to_rc()?.add_or_update_cycle_event(real_key.as_str(), &event, to, 0).await?;
            } else {
                if cur_block.number() - latest_trigger_block >= to {
                    self.ref_state.to_rc()?.add_or_update_cycle_event(real_key.as_str(), &event, to, cur_block.number() + 1).await?;
                } else {
                    self.ref_state.to_rc()?.add_or_update_cycle_event(real_key.as_str(), &event, to, latest_trigger_block + to).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn run_event(&self, block: &BlockDesc) -> BuckyResult<Vec<EventRecord>> {
        let mut record_list = Vec::new();
        let cycle_list = self.ref_state.to_rc()?.get_cycles().await?;
        for cycle in cycle_list {
            // let cycle = self.config.to_rc()?.get_rent_cycle();
            let offsets = block.number() % cycle;
            let events = self.ref_state.to_rc()?.get_cycle_events(offsets, cycle).await?;
            // if events.len() == 0 {
            //     self.ref_state.to_rc()?.delete_cycle(cycle).await?;
            // }
            for (_, start_height, event) in events {
                if start_height > block.number() {
                    continue
                }

                let listener = {
                    let map = self.listener_hash.lock().unwrap();
                    let ret = map.get(&event.get_type());
                    if ret.is_some() {
                        Some(ret.unwrap().clone())
                    } else {
                        None
                    }
                };
                if let Some(listener) = listener {
                    let ret = listener.call(block.clone(), event.clone()).await;
                    if let Err(e) = &ret {
                        if get_meta_err_code(e)? == ERROR_SKIP {
                            continue
                        }
                        return Err(ret.err().unwrap());
                    }
                    record_list.push(EventRecord::new(event, ret.unwrap()));
                }
            }
        }

        let once_events = self.ref_state.to_rc()?.get_once_events(block.number()).await?;
        self.ref_state.to_rc()?.drop_once_event(block.number()).await?;
        for event in once_events {
            let listener = {
                let map = self.listener_hash.lock().unwrap();
                let ret = map.get(&event.get_type());
                if ret.is_some() {
                    Some(ret.unwrap().clone())
                } else {
                    None
                }
            };
            if let Some(listener) = listener {
                let ret = listener.call(block.clone(), event.clone()).await;
                if let Err(e) = &ret {
                    if get_meta_err_code(e)? == ERROR_SKIP {
                        continue
                    }
                    return Err(ret.err().unwrap());
                }
                record_list.push(EventRecord::new(event, ret.unwrap()));
            }
        }

        Ok(record_list)
    }
}

#[cfg(test)]
pub mod event_manager_tests {
    use crate::{BlockDesc, sql_storage_tests, ArcWeakHelper, State};
    use crate::executor::context::{Config, ConfigRef};
    use crate::events::event_manager::EventManager;
    use cyfs_base_meta::*;
    use cyfs_base::ObjectId;
    use std::str::FromStr;

    #[test]
    fn test_add_cycle_event() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let ret = state.create_cycle_event_table(10).await;
            assert!(ret.is_ok());
            let config = Config::new(&state).unwrap();
            let event_manager = EventManager::new(&state, &config);
            let ret = event_manager.add_or_update_cycle_event("test", &Event::Rent(RentParam {
                id: ObjectId::default()
            }), 10, 2).await;
            assert!(ret.is_ok());
            let ret = event_manager.get_cycle_event("test", 10).await;
            assert!(ret.is_ok());
        });
    }

    #[test]
    fn test_add_once_event() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let ret = state.create_cycle_event_table(10).await;
            assert!(ret.is_ok());
            let config = Config::new(&state).unwrap();
            let event_manager = EventManager::new(&state, &config);
            let ret = event_manager.add_or_update_once_event("test", &Event::Rent(RentParam {
                id: ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap()
            }), 10).await;
            assert!(ret.is_ok());
            let ret = event_manager.get_once_event("test").await;
            assert!(ret.is_ok());

            let event1 = ret.unwrap();
            assert!(EventType::Rent == event1.get_type());
            if let Event::Rent(param) = event1 {
                assert_eq!(param.id, ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap());
            }

            let ret = event_manager.add_or_update_once_event("test", &Event::Rent(RentParam {
                id: ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap()
            }), 10).await;
            assert!(ret.is_ok());
            let ret = event_manager.get_once_event("test").await;
            assert!(ret.is_ok());

            let event2 = ret.unwrap();
            assert!(EventType::Rent == event2.get_type());
            if let Event::Rent(param) = event2 {
                assert_eq!(param.id, ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap());
            }
        })
    }

    #[test]
    fn test_execute_event() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let ret = event_manager.add_or_update_cycle_event("test", &Event::Rent(RentParam {
                id: ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap()
            }), config.get_rent_cycle(), 2).await;
            assert!(ret.is_ok());

            let config = Config::new(&state).unwrap();
            let event_manager = EventManager::new(&state, &config);
            let ret = event_manager.add_or_update_cycle_event("test", &Event::NameRent(NameRentParam {
                name_id: "test".to_owned()
            }), config.get_rent_cycle(), 3).await;
            assert!(ret.is_ok());

            let ret = event_manager.add_or_update_once_event("test", &Event::Rent(RentParam {
                id: ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap()
            }), 10).await;
            assert!(ret.is_ok());

            let config_ref = ConfigRef::downgrade(&config);
            event_manager.register_listener(EventType::Rent, move |cur_block: BlockDesc, event: Event| {
                let config_ref = config_ref.clone();
                Box::pin(async move {
                    match event {
                        Event::Rent(param) => {
                            assert!(param.id == ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap()
                                || param.id == ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap());

                            if param.id == ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap() {
                                assert_eq!(cur_block.number(), 10);
                            } else {
                                assert_eq!(cur_block.number() % config_ref.to_rc()?.get_rent_cycle(), 2);
                            }
                        }
                        _ => {}
                    }
                    Ok(EventResult::new(0, Vec::new()))
                })
            });

            let config_ref = ConfigRef::downgrade(&config);
            event_manager.register_listener(EventType::NameRent, move |cur_block: BlockDesc, event: Event| {
                let config_ref = config_ref.clone();
                Box::pin(async move {
                    match event {
                        Event::NameRent(param) => {
                            assert_eq!(param.name_id, "test".to_owned());
                            assert_eq!(cur_block.number() % config_ref.to_rc()?.get_rent_cycle(), 3);
                        },
                        _ => {}
                    }
                    Ok(EventResult::new(0, Vec::new()))
                })
            });

            let mut prev = BlockDesc::new(BlockDescContent::new(ObjectId::default(), None)).build();
            for _ in 1..300 {
                let new = BlockDesc::new(BlockDescContent::new(ObjectId::default(), Some(&prev))).build();
                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());
                prev = new;
            }
        })
    }
}
