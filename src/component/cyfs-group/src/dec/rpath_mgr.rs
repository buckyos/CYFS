use std::{collections::HashMap, sync::Arc};

use async_std::sync::RwLock;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, GroupId, ObjectId, RsaCPUObjectSigner};
use cyfs_bdt::Stack;
use cyfs_core::{DecAppId, GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath};

use crate::{
    storage::GroupStorage, DelegateFactory, HotstuffMessage, HotstuffPackage, IsCreateRPath,
    RPathClient, RPathControl, NET_PROTOCOL_VPORT,
};

type ControlByRPath = HashMap<String, RPathControl>;
type ControlByDec = HashMap<ObjectId, ControlByRPath>;
type ControlByGroup = HashMap<ObjectId, ControlByDec>;

type ClientByRPath = HashMap<String, RPathClient>;
type ClientByDec = HashMap<ObjectId, ClientByRPath>;
type ClientByGroup = HashMap<ObjectId, ClientByDec>;

struct GroupRPathMgrRaw {
    delegate_by_dec: HashMap<ObjectId, Box<dyn DelegateFactory>>,
    control_by_group: ControlByGroup,
    client_by_group: ClientByGroup,
}

struct LocalInfo {
    local_id: ObjectId,
    signer: Arc<RsaCPUObjectSigner>,
    network_sender: crate::network::Sender,
    non_driver: crate::network::NonDriver,
    bdt_stack: Stack,
}

#[derive(Clone)]
pub struct GroupRPathMgr(Arc<(LocalInfo, RwLock<GroupRPathMgrRaw>)>);

impl GroupRPathMgr {
    pub fn new(
        local_id: ObjectId,
        signer: RsaCPUObjectSigner,
        non_driver: crate::network::NonDriver,
        bdt_stack: Stack,
    ) -> BuckyResult<Self> {
        let datagram = bdt_stack.datagram_manager().bind(NET_PROTOCOL_VPORT)?;
        let network_sender = crate::network::Sender::new(datagram.clone(), non_driver.clone());

        let local_info = LocalInfo {
            local_id,
            signer: Arc::new(signer),
            network_sender,
            non_driver,
            bdt_stack,
        };

        let raw = GroupRPathMgrRaw {
            control_by_group: ControlByGroup::default(),
            client_by_group: ClientByGroup::default(),
            delegate_by_dec: HashMap::default(),
        };

        let mgr = Self(Arc::new((local_info, RwLock::new(raw))));

        crate::network::Listener::spawn(datagram, mgr.clone());

        Ok(mgr)
    }

    pub async fn register(
        &self,
        dec_id: DecAppId,
        delegate_factory: Box<dyn DelegateFactory>,
    ) -> BuckyResult<()> {
        let mut raw = self.write().await;
        raw.delegate_by_dec
            .insert(dec_id.object_id().clone(), delegate_factory);
        Ok(())
    }

    pub async fn unregister(&self, dec_id: &DecAppId) -> BuckyResult<()> {
        let mut raw = self.write().await;
        raw.delegate_by_dec.remove(dec_id.object_id());
        Ok(())
    }

    pub async fn find_rpath_control(
        &self,
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        is_auto_create: IsCreateRPath,
    ) -> BuckyResult<RPathControl> {
        self.find_rpath_control_inner(group_id, dec_id, rpath, is_auto_create, None, None)
            .await
    }

    pub async fn rpath_client(
        &self,
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
    ) -> BuckyResult<RPathClient> {
        {
            // read
            let raw = self.read().await;
            let found = raw
                .client_by_group
                .get(group_id)
                .map_or(None, |by_dec| by_dec.get(dec_id))
                .map_or(None, |by_rpath| by_rpath.get(rpath));

            if let Some(found) = found {
                return Ok(found.clone());
            }
        }

        {
            // write

            let local_info = self.local_info();
            let local_id = local_info.local_id;
            let network_sender = local_info.network_sender.clone();
            let non_driver = local_info.non_driver.clone();

            let mut raw = self.write().await;

            let found = raw
                .client_by_group
                .entry(group_id.clone())
                .or_insert_with(HashMap::new)
                .entry(dec_id.clone())
                .or_insert_with(HashMap::new)
                .entry(rpath.to_string());

            match found {
                std::collections::hash_map::Entry::Occupied(found) => Ok(found.get().clone()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let client = RPathClient::load(
                        local_id,
                        GroupRPath::new(group_id.clone(), dec_id.clone(), rpath.to_string()),
                        non_driver,
                        network_sender,
                    )
                    .await?;
                    entry.insert(client.clone());
                    Ok(client)
                }
            }
        }
    }

    pub async fn set_sync_path(&self, dec_id: &str, path: String) -> BuckyResult<()> {
        unimplemented!()
    }

    // return Vec<GroupId>
    pub async fn enum_group(&self) -> BuckyResult<Vec<GroupId>> {
        unimplemented!()
    }

    // return <DecId, RPath>
    pub async fn enum_rpath_control(
        &self,
        group_id: &ObjectId,
    ) -> BuckyResult<Vec<(DecAppId, String)>> {
        unimplemented!()
    }

    fn local_info(&self) -> &LocalInfo {
        &self.0 .0
    }

    async fn read(&self) -> async_std::sync::RwLockReadGuard<'_, GroupRPathMgrRaw> {
        self.0 .1.read().await
    }

    async fn write(&self) -> async_std::sync::RwLockWriteGuard<'_, GroupRPathMgrRaw> {
        self.0 .1.write().await
    }

    pub(crate) async fn on_message(
        &self,
        msg: HotstuffPackage,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        match msg {
            HotstuffPackage::Block(block) => {
                let rpath = block.r_path();
                let control = self
                    .find_rpath_control_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.r_path(),
                        IsCreateRPath::Yes(None),
                        Some(&block),
                        Some(&remote),
                    )
                    .await?;
                control
                    .on_message(HotstuffMessage::Block(block), remote)
                    .await;
            }
            HotstuffPackage::BlockVote(target, vote) => {
                let rpath = target.check_rpath();
                let control = self
                    .find_rpath_control_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.r_path(),
                        IsCreateRPath::Yes(None),
                        None,
                        Some(&remote),
                    )
                    .await?;
                control
                    .on_message(HotstuffMessage::BlockVote(vote), remote)
                    .await;
            }
            HotstuffPackage::TimeoutVote(target, vote) => {
                let rpath = target.check_rpath();
                let control = self
                    .find_rpath_control_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.r_path(),
                        IsCreateRPath::Yes(None),
                        None,
                        Some(&remote),
                    )
                    .await?;
                control
                    .on_message(HotstuffMessage::TimeoutVote(vote), remote)
                    .await;
            }
            HotstuffPackage::Timeout(target, tc) => {
                let rpath = target.check_rpath();
                let control = self
                    .find_rpath_control_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.r_path(),
                        IsCreateRPath::Yes(None),
                        None,
                        Some(&remote),
                    )
                    .await?;
                control
                    .on_message(HotstuffMessage::Timeout(tc), remote)
                    .await;
            }
            HotstuffPackage::SyncRequest(target, min_bound, max_bound) => {
                let rpath = target.check_rpath();
                let control = self
                    .find_rpath_control_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.r_path(),
                        IsCreateRPath::Yes(None),
                        None,
                        Some(&remote),
                    )
                    .await?;
                control
                    .on_message(HotstuffMessage::SyncRequest(min_bound, max_bound), remote)
                    .await;
            }
            HotstuffPackage::LastStateRequest(target) => {
                let rpath = target.check_rpath();
                let control = self
                    .find_rpath_control_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.r_path(),
                        IsCreateRPath::Yes(None),
                        None,
                        Some(&remote),
                    )
                    .await?;
                control
                    .on_message(HotstuffMessage::LastStateRequest, remote)
                    .await;
            }
            HotstuffPackage::StateChangeNotify(target, header_block, qc_block) => {
                let rpath = target.check_rpath();
                let client = self
                    .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.r_path())
                    .await?;
                client
                    .on_message(
                        HotstuffMessage::StateChangeNotify(header_block, qc_block),
                        remote,
                    )
                    .await;
            }
            HotstuffPackage::ProposalResult(target, proposal_id, result) => {
                let rpath = target.check_rpath();
                let client = self
                    .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.r_path())
                    .await?;
                client
                    .on_message(HotstuffMessage::ProposalResult(proposal_id, result), remote)
                    .await;
            }
            HotstuffPackage::QueryState(target, sub_path) => {
                let rpath = target.check_rpath();
                let client = self
                    .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.r_path())
                    .await?;
                client
                    .on_message(HotstuffMessage::QueryState(sub_path), remote)
                    .await;
            }
            HotstuffPackage::VerifiableState(target, sub_path, result) => {
                let rpath = target.check_rpath();
                let client = self
                    .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.r_path())
                    .await?;
                client
                    .on_message(HotstuffMessage::VerifiableState(sub_path, result), remote)
                    .await;
            }
        }

        Ok(())
    }

    async fn find_rpath_control_inner(
        &self,
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        is_auto_create: IsCreateRPath,
        block: Option<&GroupConsensusBlock>,
        remote: Option<&ObjectId>,
    ) -> BuckyResult<RPathControl> {
        {
            // read
            let raw = self.read().await;
            let found = raw
                .control_by_group
                .get(group_id)
                .map_or(None, |by_dec| by_dec.get(dec_id))
                .map_or(None, |by_rpath| by_rpath.get(rpath));

            if let Some(found) = found {
                return Ok(found.clone());
            }
        }

        {
            // write

            let local_info = self.local_info();
            let local_id = local_info.local_id;
            let signer = local_info.signer.clone();
            let network_sender = local_info.network_sender.clone();
            let non_driver = local_info.non_driver.clone();
            let local_device_id = local_info.bdt_stack.local_device_id().clone();

            let store = GroupStorage::load(group_id, dec_id, rpath).await;
            let store = match store {
                Ok(store) => Some(store),
                Err(e) => {
                    if let IsCreateRPath::No = is_auto_create {
                        return Err(e);
                    }
                    if let BuckyErrorCode::NotFound = e.code() {
                        None
                    } else {
                        return Err(e);
                    }
                }
            };

            // TODO: query group
            let group = self
                .local_info()
                .non_driver
                .get_group(group_id, block.map(|b| b.group_chunk_id()), remote)
                .await?;

            let mut raw = self.write().await;

            let delegate = {
                let delegate_factory = raw.delegate_by_dec.get(dec_id);
                if delegate_factory.is_none() {
                    return Err(BuckyError::new(
                        BuckyErrorCode::DecNotRunning,
                        "dec not running for the rpath-control",
                    ));
                }
                let delegate_factory = delegate_factory.unwrap();

                delegate_factory
                    .create_rpath_delegate(&group, rpath, block)
                    .await?
            };

            let store = match store {
                Some(store) => store,
                None => {
                    let init_state = match is_auto_create {
                        IsCreateRPath::Yes(init_state) => init_state,
                        _ => unreachable!(),
                    };
                    GroupStorage::create(group_id, dec_id, rpath, init_state).await?
                }
            };

            let found = raw
                .control_by_group
                .entry(group_id.clone())
                .or_insert_with(HashMap::new)
                .entry(dec_id.clone())
                .or_insert_with(HashMap::new)
                .entry(rpath.to_string());

            match found {
                std::collections::hash_map::Entry::Occupied(found) => Ok(found.get().clone()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let control = RPathControl::load(
                        local_id,
                        local_device_id.object_id().clone(),
                        GroupRPath::new(group_id.clone(), dec_id.clone(), rpath.to_string()),
                        signer,
                        Arc::new(delegate),
                        network_sender,
                        non_driver,
                        store,
                    )
                    .await?;
                    entry.insert(control.clone());
                    Ok(control)
                }
            }
        }
    }
}
