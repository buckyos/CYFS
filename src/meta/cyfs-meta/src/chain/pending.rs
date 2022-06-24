use cyfs_base::*;
use cyfs_base_meta::*;
use crate::state_storage::{State};
use std::collections::{BTreeMap, HashMap};
use crate::chain::chain_storage::ChainStorageRef;

type OrphanTxList = BTreeMap<i64, MetaTx>;
type OrphanTxMap = HashMap<ObjectId, OrphanTxList>;
type NonceMap = HashMap<ObjectId, i64>;

pub struct PendingTransactions {
    chain_storage: ChainStorageRef,
    transactions: Vec<MetaTx>,
    orphan_tx_map: OrphanTxMap,
    nonce_map: NonceMap,
    reserved: Option<fn() -> dyn State>,
}

impl PendingTransactions {
    pub fn new(chain_storage: &ChainStorageRef) -> Self {
        PendingTransactions {
            chain_storage: chain_storage.clone(),
            transactions: vec![],
            orphan_tx_map: OrphanTxMap::new(),
            nonce_map: NonceMap::new(),
            reserved: None,
        }
    }

    fn is_exist(&self, tx: &MetaTx) -> BuckyResult<bool> {
        for exists in &self.transactions {
            if tx.desc().calculate_id() == exists.desc().calculate_id() {
                return Ok(true);
            }
        }

        if !self.orphan_tx_map.contains_key(&tx.desc().content().caller.id()?) {
            return Ok(false);
        }

        for (k, _) in &self.orphan_tx_map {
            if k == &tx.desc().calculate_id() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub async fn get_nonce(&self, id: &ObjectId) -> BuckyResult<i64> {
        if self.nonce_map.contains_key(id) {
            let nonce = self.nonce_map.get(id).unwrap();
            Ok(*nonce)
        } else {
            let (_, tip_storage) = self.chain_storage.get_tip_info().await?;
            let state = tip_storage.create_state(true).await;
            state.get_nonce(id).await
        }
    }

    pub async fn push(&mut self, tx: MetaTx) -> BuckyResult<()> {
        // if !tx.tx().verify_signature()? {
        //     return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "InvalidInput"));
        // }

        if self.is_exist(&tx)? {
            log::error!("add to pending transactions failed for same caller exists");
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, "InvalidParam"));
        }

        let caller_id = tx.desc().content().caller.id()?;
        let mut nonce = self.get_nonce(&caller_id).await?;
        nonce += 1;
        if tx.desc().content().nonce == nonce {
            self.nonce_map.insert(caller_id, nonce);
            self.transactions.push(tx);
        } else if tx.desc().content().nonce > nonce {
            self.add_to_orphan(tx)?;
            return Ok(());
        } else {
            let tx_nonce = tx.desc().content().nonce;
            if !self.try_update_same_nonce_tx(tx)? {
                log::error!("add to pending transactions failed for invalid nonce, expeced {:?} but {:?}", nonce, tx_nonce);
                return Err(crate::meta_err!(ERROR_INVALID));
            }
        }

        if self.orphan_tx_map.contains_key(&caller_id) {
            let list = self.orphan_tx_map.get_mut(&caller_id).unwrap();
            loop {
                nonce += 1;
                if list.contains_key(&nonce) {
                    let tx = list.remove(&nonce).unwrap();
                    self.nonce_map.insert(caller_id, nonce);
                    self.transactions.push(tx);
                } else {
                    break;
                }
            }
        }
        // log::info!("add tx {:?} to pending transactions", tx.desc().calculate_id().to_string());
        Ok(())
    }

    fn add_to_orphan(&mut self, tx: MetaTx) -> BuckyResult<()> {
        let caller_id = tx.desc().content().caller.id()?;
        if !self.orphan_tx_map.contains_key(&caller_id) {
            self.orphan_tx_map.insert(caller_id.clone(), OrphanTxList::new());
        }
        let list = self.orphan_tx_map.get_mut(&caller_id).unwrap();
        list.insert(tx.desc().content().nonce, tx);
        Ok(())
    }

    fn try_update_same_nonce_tx(&mut self, tx: MetaTx) -> BuckyResult<bool> {
        let caller_id = tx.desc().content().caller.id()?;
        for (i, exist) in self.transactions.iter().enumerate() {
            if caller_id == exist.desc().content().caller.id()? && &tx.desc().content().nonce == &exist.desc().content().nonce {
                self.transactions[i] = tx;
                return Ok(true)
            }
        }

        if self.orphan_tx_map.contains_key(&caller_id) {
            let list = self.orphan_tx_map.get_mut(&caller_id).unwrap();
            let nonce = tx.desc().content().nonce;
            if list.contains_key(&nonce) {
                list.remove(&nonce);
                list.insert(nonce, tx);
            }
        }

        Ok(false)
    }

    pub fn pop_all(&mut self) -> BuckyResult<Vec<MetaTx>> {
        let mut all = vec![];
        let mut op_tx = self.transactions.pop();
        while op_tx.is_some() {
            all.insert(0, op_tx.unwrap());
            op_tx = self.transactions.pop();
        }
        Ok(all)
    }

    pub fn get_all(&self) -> BuckyResult<Vec<MetaTx>> {
        let mut all = vec![];
        let mut i = 0;
        for tx in &self.transactions {
            i += 1;
            all.push(tx.clone());
            if i >= 40 {
                break;
            }
        }
        Ok(all)
    }

    pub fn exists(&self, hash: &TxHash) -> bool {
        for transaction in &self.transactions {
            if hash == &transaction.desc().calculate_id() {
                return true;
            }
        }
        false
    }

    pub async fn remove(&mut self, tx: &MetaTx) -> BuckyResult<()> {
        let hash = tx.desc().calculate_id();
        let caller_id = tx.desc().content().caller.id()?;
        let mut nonce = self.get_nonce(&caller_id).await?;
        if tx.desc().content().nonce > nonce {
            nonce = tx.desc().content().nonce;
            self.nonce_map.insert(caller_id, nonce);

            if self.orphan_tx_map.contains_key(&caller_id) {
                let list = self.orphan_tx_map.get_mut(&caller_id).unwrap();
                loop {
                    nonce += 1;
                    if list.contains_key(&nonce) {
                        let tx = list.remove(&nonce).unwrap();
                        self.nonce_map.insert(caller_id, nonce);
                        self.transactions.push(tx);
                    } else {
                        break;
                    }
                }
            }
        }

        let mut index = 0;
        for transaction in &self.transactions {
            if hash == transaction.desc().calculate_id() {
                self.transactions.remove(index);
                break;
            }
            index += 1;
        }

        Ok(())
    }

    pub fn get_tx(&self, hash: &TxHash) -> Option<MetaTx> {
        for transaction in &self.transactions {
            if hash == &transaction.desc().calculate_id() {
                return Some(transaction.clone());
            }
        }
        None
    }
}

#[cfg(test)]
pub mod pending_transactions_test {
    use crate::{BlockDesc};
    use crate::chain::pending::PendingTransactions;
    use cyfs_base::*;
    use std::convert::TryFrom;
    use cyfs_base_meta::*;
    use crate::chain::chain_storage::chain_storage_tests::create_test_chain_storage;

    pub fn create_test_tx(people: &StandardObject, nonce: i64) -> MetaTx {
        let body = MetaTxBody::TransBalance(TransBalanceTx {
            ctid: CoinTokenId::Coin(0),
            to: vec![]
        });
        let tx = MetaTx::new(nonce, TxCaller::try_from(people).unwrap()
                            , 0
                            , 0
                            , 0
                            , None
                            , body, Vec::new()).build();
        tx
    }

    #[test]
    fn test() {
        async_std::task::block_on(async {
            let chain_storage = create_test_chain_storage("test4").await;

            let _header = BlockDesc::new(BlockDescContent::new(ObjectId::default(), None)).build();
            let mut pending = PendingTransactions::new(&chain_storage);

            let private_key = PrivateKey::generate_rsa(1024).unwrap();
            let public_key = private_key.public();
            let people = StandardObject::Device(Device::new(None
                                                            , UniqueId::default()
                                                            , Vec::new()
                                                            , Vec::new()
                                                            , Vec::new()
                                                            , public_key
                                                            , Area::default()
                                                            , DeviceCategory::OOD).build());
            // let tx = create_test_tx(&people, 1);
            // let ret = pending.push(MetaTx::new(tx).unwrap());
            // assert!(!ret.is_ok());

            let mut tx = create_test_tx(&people, 1);
            tx.sign(private_key.clone()).unwrap();
            let tx_id1 = tx.desc().calculate_id();
            let ret = pending.push(tx).await;
            if let Err(e) = &ret {
                let msg = format!("{:?}", e);
                println!("{}", msg);
            }
            assert!(ret.is_ok());

            let mut tx = create_test_tx(&people, 1);
            tx.sign(private_key.clone()).unwrap();
            let tx_id2 = tx.desc().calculate_id();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let tx_list = pending.pop_all().unwrap();
            assert_eq!(tx_list.len(), 1);
            assert_ne!(tx_list.get(0).unwrap().desc().calculate_id(), tx_id1);
            assert_eq!(tx_list.get(0).unwrap().desc().calculate_id(), tx_id2);

            let mut tx = create_test_tx(&people, 1);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(!ret.is_ok());

            let mut tx = create_test_tx(&people, 2);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let mut tx = create_test_tx(&people, 5);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let mut tx = create_test_tx(&people, 6);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let tx_list = pending.pop_all().unwrap();
            assert_eq!(tx_list.len(), 1);

            let mut tx = create_test_tx(&people, 3);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let mut tx = create_test_tx(&people, 4);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let tx_list = pending.pop_all().unwrap();
            assert_eq!(tx_list.len(), 4);

        });
    }

    #[test]
    fn test_remove() {
        async_std::task::block_on(async {
            let chain_storage = create_test_chain_storage("test5").await;

            let _header = BlockDesc::new(BlockDescContent::new(ObjectId::default(), None)).build();
            let mut pending = PendingTransactions::new(&chain_storage);

            let private_key = PrivateKey::generate_rsa(1024).unwrap();
            let public_key = private_key.public();
            let people = StandardObject::Device(Device::new(None
                                                            , UniqueId::default()
                                                            , Vec::new()
                                                            , Vec::new()
                                                            , Vec::new()
                                                            , public_key
                                                            , Area::default()
                                                            , DeviceCategory::OOD).build());


            let mut tx1 = create_test_tx(&people, 1);
            tx1.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx1.clone()).await;
            assert!(ret.is_ok());


            let mut tx = create_test_tx(&people, 3);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            pending.remove(&tx1).await.unwrap();

            let tx_list = pending.pop_all().unwrap();
            assert_eq!(tx_list.len(), 0);

            let mut tx = create_test_tx(&people, 1);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(!ret.is_ok());

            let mut tx = create_test_tx(&people, 2);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let tx_list = pending.pop_all().unwrap();
            assert_eq!(tx_list.len(), 2);



            let mut tx = create_test_tx(&people, 6);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let mut tx = create_test_tx(&people, 7);
            tx.sign(private_key.clone()).unwrap();
            let ret = pending.push(tx).await;
            assert!(ret.is_ok());

            let mut tx = create_test_tx(&people, 5);
            tx.sign(private_key.clone()).unwrap();
            pending.remove(&tx).await.unwrap();

            let ret = pending.get_nonce(&people.calculate_id()).await;
            assert!(ret.is_ok());
            assert_eq!(ret.unwrap(), 7 as i64);

            let tx_list = pending.pop_all().unwrap();
            assert_eq!(tx_list.len(), 2);
        });
    }
}
