use cyfs_base::*;
use std::collections::{HashMap, LinkedList};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::{Arc};
use std::cell::RefCell;
use std::rc::Rc;
use cyfs_debug::Mutex;


const MIX_HASH_LIVE_MINUTES: usize = 3;

#[derive(Clone)]
pub struct Keystore {
    local_encryptor: Arc<(PrivateKey, DeviceDesc, RsaCPUObjectSigner)>,
    key_manager: Arc<Mutex<KeyManager>>,
}

unsafe impl Send for Keystore {}
unsafe impl Sync for Keystore {}

pub struct FoundKey {
    pub aes_key: AesKey,
    pub peerid: DeviceId,
    pub hash: KeyMixHash,
    pub is_confirmed: bool,
}

#[derive(Clone)]
pub struct Config {
    pub active_time: Duration,
    pub capacity: usize,
}

impl Keystore {
    pub fn new(
        private_key: PrivateKey, 
        const_info: DeviceDesc, 
        signer: RsaCPUObjectSigner, 
        config: Config) -> Self {
        Keystore {
            local_encryptor: Arc::new((private_key, const_info, signer)),
            key_manager: Arc::new(Mutex::new(KeyManager::new(config))),
        }
    }

    pub fn private_key(&self) -> &PrivateKey {
        &self.local_encryptor.0
    }

    pub fn signer(&self) -> &RsaCPUObjectSigner {
        &self.local_encryptor.2
    }

    pub fn public_key(&self) -> &PublicKey { self.local_encryptor.1.public_key() }

    pub fn get_key_by_remote(&self, peerid: &DeviceId, is_touch: bool) -> Option<FoundKey> {
        let mut mgr = self.key_manager.lock().unwrap();
        mgr.find_by_peerid(peerid, is_touch).map(|found| found.as_ref().borrow().found())
    }

    pub fn get_key_by_mix_hash(&self, mix_hash: &KeyMixHash, is_touch: bool, is_confirmed: bool) -> Option<FoundKey> {
        let mut mgr = self.key_manager.lock().unwrap();
        mgr.find_by_mix_hash(mix_hash, is_touch, is_confirmed).map(|found| found.as_ref().borrow().found())
    }

    pub fn create_key(&self, peerid: &DeviceId, is_touch: bool) -> FoundKey {
        let mut mgr = self.key_manager.lock().unwrap();
        match mgr.find_by_peerid(peerid, is_touch) {
            Some(found) => found.as_ref().borrow().found(),
            None => {
                let new_key = AesKey::random();
                mgr.add_key(&new_key, peerid, false, true);
                let hash = new_key.mix_hash(None);
                FoundKey {
                    aes_key: new_key,
                    peerid: peerid.clone(),
                    hash,
                    is_confirmed: false
                }
            }
        }
    }

    pub fn add_key(&self, key: &AesKey, remote: &DeviceId, is_confirmed: bool) {
        let mut mgr = self.key_manager.lock().unwrap();
        mgr.add_key(key, remote, is_confirmed, false)
    }

    pub fn reset_peer(&self, device_id: &DeviceId) {
        let mut mgr = self.key_manager.lock().unwrap();
        mgr.reset_peer(device_id);
    }
}

struct KeyManager {
    // ???peerid?????????key
    peerid_key_map: HashMap<DeviceId, Vec<Rc<RefCell<HashedKeyInfo>>>>,
    // ???hash?????????key
    mixhash_key_map: HashMap<KeyMixHash, Rc<RefCell<HashedKeyInfo>>>,
    // ????????????key???????????????mixhash??????????????????????????????????????????hash
    latest_use_key_list: LatestUseKeyList,
    // hash?????????key???????????????
    timeout_hash_key_list: LinkedList<Rc<RefCell<HashedKeyInfo>>>,
    // ??????????????????hash
    will_drop_hash_list: LinkedList<KeyMixHash>,

    config: Config,
}

impl KeyManager {
    fn new(config: Config) -> Self {
        KeyManager {
            peerid_key_map: HashMap::default(),
            mixhash_key_map: HashMap::default(),
            latest_use_key_list: LatestUseKeyList::new(),
            timeout_hash_key_list: Default::default(),
            will_drop_hash_list: Default::default(),
            config
        }
    }

    fn find_by_mix_hash(&mut self, mix_hash: &KeyMixHash, is_touch: bool, is_confirmed: bool) -> Option<Rc<RefCell<HashedKeyInfo>>> {
        let mut found = None;
        let now = SystemTime::now();

        let found_map = self.mixhash_key_map.get(mix_hash);
        if let Some(target) = found_map {
            let target = target.clone();
            self.rehash(target.clone());
            found = Some(target);
        } else {
            let minute_timestamp = now.duration_since(UNIX_EPOCH).unwrap().as_secs() / 60;
            let mut timeout_keys = self.latest_use_key_list.try_remove_all(minute_timestamp);
            timeout_keys.append(&mut self.timeout_hash_key_list);
            self.timeout_hash_key_list = timeout_keys;

            // ????????????hash????????????????????????????????????????????????????????????
            while let Some(rehash) = self.timeout_hash_key_list.pop_front() {
                if rehash.as_ref().borrow().info.expire_time <= now {
                    continue;
                }
                let new_hashs = self.rehash(rehash.clone());
                if new_hashs.iter().find(|h| **h == *mix_hash).is_some() {
                    self.latest_use_key_list.push_front(rehash.clone());
                    found = Some(rehash.clone());
                    break;
                } else {
                    self.latest_use_key_list.push_back(rehash.clone());
                }
            }

            self.check_mixhash_capacity();
        }

        let mut _is_changed = false;
        if let Some(target) = found {
            log::trace!("found by mix-hash: {}", mix_hash.to_string());

            let target_info = &mut target.as_ref().borrow_mut().info;
            target_info.last_access_time = now;
            let expire_time = if is_touch { now + self.config.active_time } else { target_info.expire_time };
            _is_changed = target_info.update(is_confirmed, expire_time);
            Some(target.clone())

            // <TODO>?????????
        } else {
            log::trace!("found by mix-hash: {} failed.", mix_hash.to_string());
            None
        }
    }

    fn find_by_peerid(&mut self, peerid: &DeviceId, is_touch: bool) -> Option<Rc<RefCell<HashedKeyInfo>>> {
        let found_map = self.peerid_key_map.get_mut(peerid);
        if let Some(found_key_list) = found_map {
            if found_key_list.len() == 0 {
                return None;
            }

            let now = SystemTime::now();
            let mut expired = Vec::with_capacity(found_key_list.len());
            let mut active: Vec<Rc<RefCell<HashedKeyInfo>>> = Default::default();
            let mut last = found_key_list.first().unwrap().clone();
            for key in found_key_list.iter() {
                if now >= key.as_ref().borrow().info.expire_time {
                    expired.push(key.clone());
                } else {
                    active.push(key.clone());
                    if key.as_ref().borrow().info.last_access_time > last.as_ref().borrow().info.last_access_time {
                        last = key.clone();
                    }
                }
            }

            *found_key_list = active;

            if found_key_list.len() == 0 {
                self.peerid_key_map.remove(peerid);
            }
            // ??????
            for rm in expired {
                let rm = rm.as_ref().borrow();
                self.mixhash_key_map.remove(&rm.original_hash);
                for mix_hash in rm.mix_hash.as_slice() {
                    self.mixhash_key_map.remove(&mix_hash.hash);
                }
            }

            let last_info = &mut last.as_ref().borrow_mut().info;
            if now < last_info.expire_time {
                last_info.last_access_time = now;
                let expire_time = if is_touch { now + self.config.active_time } else { last_info.expire_time };
                let _is_changed = last_info.update(false, expire_time);

                // <TODO>?????????

                Some(last.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    fn add_key(&mut self, aes_key: &AesKey, peerid: &DeviceId, is_confirmed: bool, is_new_key: bool) {
        let now = SystemTime::now();
        let expire_time = now + self.config.active_time;

        let target_peer_key_list = self.peerid_key_map.entry(peerid.clone()).or_insert(Vec::default());

        let mut target_key = None;
        if !is_new_key { // ????????????key???????????????
            target_key = target_peer_key_list.iter().find(|k| k.as_ref().borrow().info.aes == *aes_key).map(|f| f.clone());
        }

        let is_new_key = target_key.is_none();
        let target_key = match target_key {
            Some(exist) => {
                let target = exist.clone();
                let mut exist = exist.as_ref().borrow_mut();
                let info = &mut exist.info;
                info.last_access_time = now;
                let _is_changed = info.update(is_confirmed, expire_time);
                // <TODO>?????????
                target
            },
            None => {
                let new_key = KeyInfo {
                    aes: aes_key.clone(),
                    peerid: peerid.clone(),
                    is_confirmed,
                    is_storaged: false,
                    expire_time: expire_time,
                    last_access_time: now
                };

                let hash = aes_key.mix_hash(None);

                Rc::new(RefCell::new(HashedKeyInfo {
                    info: new_key,
                    original_hash: hash,
                    mix_hash: vec![]
                }))

                // <TODO>?????????
            }
        };

        if is_new_key {
            log::trace!("create new key mix-hash: {}, remote: {}", target_key.as_ref().borrow().original_hash.to_string(), peerid);

            target_peer_key_list.push(target_key.clone());
            self.latest_use_key_list.push_front(target_key.clone());
            self.mixhash_key_map.insert(target_key.as_ref().borrow().original_hash.clone(), target_key.clone());
        }

        self.rehash(target_key);
        self.check_key_capacity();
    }

    fn rehash(&mut self, target: Rc<RefCell<HashedKeyInfo>>) -> Vec<KeyMixHash> {
        let (new_hash, drop_hash) = target.as_ref().borrow_mut().rehash();
        for add in new_hash.as_slice() {
            self.mixhash_key_map.insert(add.clone(), target.clone());
        }

        for rm in drop_hash {
            self.will_drop_hash_list.push_back(rm);
        }

        new_hash
    }

    fn check_mixhash_capacity(&mut self) {
        // ??????
        let max_hash_count = self.config.capacity * (MIX_HASH_LIVE_MINUTES + 1) * 5 / 4;
        let hash_count = self.mixhash_key_map.len();
        if hash_count >= max_hash_count {
            let mut drop_hashs = Default::default();
            std::mem::swap(&mut self.will_drop_hash_list, &mut drop_hashs);
            for hash in drop_hashs {
                self.mixhash_key_map.remove(&hash);
            }
        }
    }

    fn check_key_capacity(&mut self) {
        self.check_mixhash_capacity();

        // ??????
        let max_key_count = (self.config.capacity * 5 / 4) as usize;
        let key_count = self.latest_use_key_list.count() + self.timeout_hash_key_list.len();
        if key_count >= max_key_count {
            let mut drop_count = max_key_count - self.config.capacity as usize;
            while drop_count > 0 && !self.timeout_hash_key_list.is_empty() {
                let key = self.timeout_hash_key_list.pop_back().unwrap();
                self.remove_key(&*key.as_ref().borrow());
                drop_count -= 1;
            }
            while drop_count > 0 && self.latest_use_key_list.count() > 0 {
                let key = self.latest_use_key_list.pop_back().unwrap();
                self.remove_key(&*key.as_ref().borrow());
                drop_count -= 1;
            }
        }
    }

    fn remove_key(&mut self, key: &HashedKeyInfo) {
        self.mixhash_key_map.remove(&key.original_hash);
        for hash in key.mix_hash.as_slice() {
            self.mixhash_key_map.remove(&hash.hash);
        }
        let peer_key_list = self.peerid_key_map.get_mut(&key.info.peerid);
        if peer_key_list.is_none() {
            return;
        }
        let peer_key_list = peer_key_list.unwrap();
        for i in 0..peer_key_list.len() {
            let idx = peer_key_list.len() - i - 1;
            let check_key = peer_key_list.get(idx).unwrap();
            if check_key.as_ref().borrow().info.aes == key.info.aes {
                peer_key_list.remove(idx);
                break;
            }
        }
        if peer_key_list.is_empty() {
            self.peerid_key_map.remove(&key.info.peerid);
        }
    }

    fn reset_peer(&mut self, device_id: &DeviceId) {
        let found_map = self.peerid_key_map.get_mut(device_id);
        if let Some(found_key_list) = found_map {
            for exist in found_key_list {
                let mut exist = exist.as_ref().borrow_mut();
                let info = &mut exist.info;
                info.is_confirmed = false;
            }   
        }
    }
}

struct LatestUseKeyList {
    // ???????????????key?????????(?????????)
    key_list: LinkedList<Rc<RefCell<HashedKeyInfo>>>,
    // ?????????hash????????????????????????
    first_hash_minute_timestamp: u64,
}

impl LatestUseKeyList {
    fn new() -> LatestUseKeyList {
        LatestUseKeyList {
            key_list: Default::default(),
            first_hash_minute_timestamp: 0
        }
    }

    fn try_remove_all(&mut self, now_minute_timestamp: u64) -> LinkedList<Rc<RefCell<HashedKeyInfo>>> {
        if now_minute_timestamp != self.first_hash_minute_timestamp {
            let mut ret = LinkedList::default();
            std::mem::swap(&mut self.key_list, &mut ret);
            self.first_hash_minute_timestamp = 0;
            ret
        } else {
            LinkedList::default()
        }
    }

    fn push_front(&mut self, key_info: Rc<RefCell<HashedKeyInfo>>) {
        self.key_list.push_front(key_info);
        if self.key_list.len() == 1 {
            self.first_hash_minute_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 60;
        }
    }

    fn push_back(&mut self, key_info: Rc<RefCell<HashedKeyInfo>>) {
        self.key_list.push_back(key_info);
        if self.key_list.len() == 1 {
            self.first_hash_minute_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 60;
        }
    }

    fn pop_back(&mut self) -> Option<Rc<RefCell<HashedKeyInfo>>> {
        self.key_list.pop_back()
    }

    fn count(&self) -> usize {
        self.key_list.len()
    }
}

struct HashedKeyInfo {
    info: KeyInfo,
    original_hash: KeyMixHash,
    mix_hash: Vec<HashInfo>,
}

impl HashedKeyInfo {
    // ??????hash, ??????(??????hash,??????hash)
    fn rehash(&mut self) -> (Vec<KeyMixHash>, Vec<KeyMixHash>) {
        let minute_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 60;
        let min = minute_timestamp - (MIX_HASH_LIVE_MINUTES as u64 - 1) / 2;
        let max = minute_timestamp + (MIX_HASH_LIVE_MINUTES as u64 - 1) / 2;

        let mut timeout_count = 0;
        let mut next_timestamp = min;
        for h in self.mix_hash.as_slice() {
            if h.minute_timestamp < min {
                timeout_count += 1;
            } else if h.minute_timestamp >= next_timestamp {
                next_timestamp = h.minute_timestamp + 1;
            }
        };

        let removed: Vec<HashInfo> = self.mix_hash.splice(..timeout_count, vec![].iter().cloned()).collect();
        let removed = removed.iter().map(|hi| hi.hash.clone()).collect();

        let mut added = vec![];
        if next_timestamp < max {
            for t in next_timestamp..(max + 1) {
                let hash = HashInfo {
                    hash: self.info.aes.mix_hash(Some(t)),
                    minute_timestamp: t
                };
                added.push(hash.hash.clone());
                self.mix_hash.push(hash);
            }
        }

        (added, removed)
    }

    fn found(&self) -> FoundKey {
        FoundKey {
            aes_key: self.info.aes.clone(),
            peerid: self.info.peerid.clone(),
            hash: self.original_hash.clone(), // <TODO>????????????hash
            is_confirmed: self.info.is_confirmed
        }
    }
}

struct KeyInfo {
    aes: AesKey,
    peerid: DeviceId,
    is_confirmed: bool,
    is_storaged: bool,
    expire_time: SystemTime,
    last_access_time: SystemTime,
}

impl KeyInfo {
    fn update(&mut self, is_confirmed: bool, expire_time: SystemTime) -> bool {
        let mut is_changed = false;
        if is_confirmed && !self.is_confirmed {
            self.is_confirmed = true;
            is_changed = true;
        }

        // ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
        if expire_time < self.expire_time {
            self.expire_time = expire_time;
            is_changed = true;
        } else {
            let now = SystemTime::now();
            if self.expire_time >= now {
                let delta = expire_time.duration_since(self.expire_time).unwrap();
                if delta >= Duration::from_secs(60) || self.expire_time.duration_since(now).unwrap() < delta {
                    self.expire_time = expire_time;
                    is_changed = true;
                }
            } else {
                self.expire_time = expire_time;
                is_changed = true;
            }
        }

        is_changed
    }
}

#[derive(Clone)]
struct HashInfo {
    hash: KeyMixHash,
    minute_timestamp: u64,
}


#[test]
fn add_key() {
    use std::time::Duration;
    // ??????peer???key????????????????????????
    let private_key = PrivateKey::generate_rsa(1024).unwrap();
    let device = Device::new(
        None,
        UniqueId::default(),
        vec![],
        vec![],
        vec![],
        private_key.public(),
        Area::default(), 
        DeviceCategory::PC
    ).build();
    let sim_device_id = device.desc().device_id();

    let signer = RsaCPUObjectSigner::new(
        private_key.public(),
        private_key.clone(),
    );

    let key_store = Keystore::new(
        private_key, 
        device.desc().clone(), 
        signer, 
        Config {
            active_time: Duration::from_secs(1),
            capacity: 5,
        });
    
    let key_for_id0_first = key_store.create_key(&sim_device_id, true);
    assert!(!key_for_id0_first.is_confirmed);
    assert_eq!(key_for_id0_first.peerid, sim_device_id);
    
    fn found_key_is_same(left: &FoundKey, right: &FoundKey) -> bool {
        left.aes_key == right.aes_key &&
            left.peerid == right.peerid &&
            left.hash == right.hash // <TODO>????????????hash???????????????
    }
    assert!(found_key_is_same(&key_store.get_key_by_remote(&sim_device_id, true).unwrap(), &key_for_id0_first));
    assert!(found_key_is_same(&key_store.get_key_by_mix_hash(&key_for_id0_first.hash, true, false).unwrap(), &key_for_id0_first));
    
    let key_for_id0_twice = key_store.create_key(&sim_device_id, true); // ???????????????key
    assert!(!key_for_id0_twice.is_confirmed);
    assert!(found_key_is_same(&key_for_id0_twice, &key_for_id0_first));
    
    let found_by_hash = key_store.get_key_by_mix_hash(&key_for_id0_first.hash, true, true).unwrap(); // confirm: false->true
    assert!(found_by_hash.is_confirmed);
    assert!(found_key_is_same(&found_by_hash, &key_for_id0_first));
    
    let found_by_hash = key_store.get_key_by_mix_hash(&key_for_id0_first.hash, true, false).unwrap(); // confirm?????????true->false
    assert!(found_by_hash.is_confirmed);
    assert!(found_key_is_same(&found_by_hash, &key_for_id0_first));
    
    let key_random = AesKey::random();
    let found_key_for_random = FoundKey {
        aes_key: key_random.clone(),
        hash: key_random.mix_hash(None),
        peerid: sim_device_id.clone(),
        is_confirmed: false
    };
    key_store.add_key(&key_random, &sim_device_id, false);
    let found_after_add = key_store.get_key_by_remote(&sim_device_id, true).unwrap();
    assert!(found_key_is_same(&found_after_add, &key_for_id0_first) || found_key_is_same(&found_after_add, &found_key_for_random)); // ??????????????????????????????????????????????????????
    let found_by_hash_after_add = key_store.get_key_by_mix_hash(&found_key_for_random.hash, true, false).unwrap();
    assert!(!found_by_hash_after_add.is_confirmed);
    assert!(found_key_is_same(&found_by_hash_after_add, &found_key_for_random));
    
    key_store.add_key(&key_random, &sim_device_id, true); // confirm: false->true
    let found_by_hash_after_add_with_confirm = key_store.get_key_by_mix_hash(&found_key_for_random.hash, true, false).unwrap();
    assert!(found_by_hash_after_add_with_confirm.is_confirmed);
    assert!(found_key_is_same(&found_by_hash_after_add_with_confirm, &found_key_for_random));
    
    let key_random2 = AesKey::random();
    let found_key_for_random2 = FoundKey {
        aes_key: key_random2.clone(),
        hash: key_random2.mix_hash(None),
        peerid: sim_device_id.clone(),
        is_confirmed: false
    };
    key_store.add_key(&key_random2, &sim_device_id, true); // ?????????add???confirm
    let found_by_hash_after_add2_with_confirm = key_store.get_key_by_mix_hash(&found_key_for_random2.hash, true, false).unwrap();
    assert!(found_by_hash_after_add2_with_confirm.is_confirmed);
    assert!(found_key_is_same(&found_by_hash_after_add2_with_confirm, &found_key_for_random2));
}
