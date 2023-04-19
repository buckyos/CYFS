use std::{collections::HashMap, sync::Arc};

use async_std::sync::RwLock;
use cyfs_base::{
    BuckyErrorCode, BuckyResult, GroupId, NamedObject, ObjectDesc, ObjectId, OwnerObjectDesc,
    RawConvertTo, RawFrom, RsaCPUObjectSigner, TypelessCoreObject,
};
use cyfs_bdt::{DatagramTunnelGuard, StackGuard};
use cyfs_core::{DecAppId, GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath};
use cyfs_group_lib::{GroupCommand, GroupCommandNewRPath};
use cyfs_lib::{GlobalStateManagerRawProcessorRef, NONObjectInfo};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};

use crate::{
    storage::{GroupShellManager, GroupStorage},
    HotstuffMessage, HotstuffPackage, NONDriver, NONDriverHelper, RPathClient, RPathEventNotifier,
    RPathService, NET_PROTOCOL_VPORT,
};

type ServiceByRPath = HashMap<String, RPathService>;
type ServiceByDec = HashMap<ObjectId, ServiceByRPath>;
type ServiceByGroup = HashMap<ObjectId, ServiceByDec>;

type ClientByRPath = HashMap<String, RPathClient>;
type ClientByDec = HashMap<ObjectId, ClientByRPath>;
type ClientByGroup = HashMap<ObjectId, ClientByDec>;

struct GroupRPathMgrRaw {
    service_by_group: ServiceByGroup,
    client_by_group: ClientByGroup,
    shell_mgr: HashMap<ObjectId, GroupShellManager>,
}

struct LocalInfo {
    signer: Arc<RsaCPUObjectSigner>,
    non_driver: Arc<Box<dyn NONDriver>>,
    datagram: DatagramTunnelGuard,
    bdt_stack: StackGuard,
    global_state_mgr: GlobalStateManagerRawProcessorRef,
    meta_client: Arc<MetaClient>,
}

#[derive(Clone)]
pub struct GroupManager(Arc<(LocalInfo, RwLock<GroupRPathMgrRaw>)>);

impl GroupManager {
    pub fn new(
        signer: RsaCPUObjectSigner,
        non_driver: Box<dyn crate::network::NONDriver>,
        bdt_stack: StackGuard,
        global_state_mgr: GlobalStateManagerRawProcessorRef,
    ) -> BuckyResult<Self> {
        let datagram = bdt_stack.datagram_manager().bind(NET_PROTOCOL_VPORT)?;
        let local_device_id = bdt_stack.local_device_id().object_id().clone();
        let metaclient = MetaClient::new_target(MetaMinerTarget::default());

        let local_info = LocalInfo {
            signer: Arc::new(signer),
            non_driver: Arc::new(non_driver),
            datagram: datagram.clone(),
            bdt_stack,
            global_state_mgr,
            meta_client: Arc::new(metaclient),
        };

        let raw = GroupRPathMgrRaw {
            service_by_group: ServiceByGroup::default(),
            client_by_group: ClientByGroup::default(),
            shell_mgr: HashMap::default(),
        };

        let mgr = Self(Arc::new((local_info, RwLock::new(raw))));

        crate::network::Listener::spawn(datagram, mgr.clone(), local_device_id);

        Ok(mgr)
    }

    pub async fn find_rpath_service(
        &self,
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        is_auto_create: bool,
    ) -> BuckyResult<RPathService> {
        self.find_rpath_service_inner(group_id, dec_id, rpath, is_auto_create, None, None)
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
            let local_id = local_info.bdt_stack.local_const().owner().unwrap();
            let local_device_id = local_info.bdt_stack.local_device_id();
            let state_mgr = local_info.global_state_mgr.clone();
            let non_driver = NONDriverHelper::new(
                local_info.non_driver.clone(),
                dec_id.clone(),
                local_device_id.object_id().clone(),
            );
            let network_sender = crate::network::Sender::new(
                local_info.datagram.clone(),
                non_driver.clone(),
                local_device_id.object_id().clone(),
            );

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
                    let state_proccessor = state_mgr
                        .load_root_state(local_device_id.object_id(), Some(local_id), true)
                        .await?;
                    let shell_mgr = self
                        .check_group_shell_mgr(group_id, non_driver.clone(), None)
                        .await?;
                    let client = RPathClient::load(
                        local_device_id.object_id().clone(),
                        GroupRPath::new(group_id.clone(), dec_id.clone(), rpath.to_string()),
                        state_proccessor.unwrap(),
                        non_driver,
                        shell_mgr,
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
    pub async fn enum_rpath_service(
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
                let rpath = block.rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        Some(&block),
                        Some(&remote),
                    )
                    .await?;
                service
                    .on_message(HotstuffMessage::Block(block), remote)
                    .await;
            }
            HotstuffPackage::BlockVote(target, vote) => {
                let rpath = target.check_rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        None,
                        Some(&remote),
                    )
                    .await?;
                service
                    .on_message(HotstuffMessage::BlockVote(vote), remote)
                    .await;
            }
            HotstuffPackage::TimeoutVote(target, vote) => {
                let rpath = target.check_rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        None,
                        Some(&remote),
                    )
                    .await?;
                service
                    .on_message(HotstuffMessage::TimeoutVote(vote), remote)
                    .await;
            }
            HotstuffPackage::Timeout(target, tc) => {
                let rpath = target.check_rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        None,
                        Some(&remote),
                    )
                    .await?;
                service
                    .on_message(HotstuffMessage::Timeout(tc), remote)
                    .await;
            }
            HotstuffPackage::SyncRequest(target, min_bound, max_bound) => {
                let rpath = target.check_rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        None,
                        Some(&remote),
                    )
                    .await?;
                service
                    .on_message(HotstuffMessage::SyncRequest(min_bound, max_bound), remote)
                    .await;
            }
            HotstuffPackage::LastStateRequest(target) => {
                let rpath = target.check_rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        None,
                        Some(&remote),
                    )
                    .await?;
                service
                    .on_message(HotstuffMessage::LastStateRequest, remote)
                    .await;
            }
            HotstuffPackage::StateChangeNotify(header_block, qc_block) => {
                // TODO: unimplemented
                // let rpath = header_block.rpath();
                // let client = self
                //     .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.rpath())
                //     .await?;
                // client
                //     .on_message(
                //         HotstuffMessage::StateChangeNotify(header_block, qc_block),
                //         remote,
                //     )
                //     .await;
            }
            HotstuffPackage::ProposalResult(proposal_id, result) => {
                // TODO: unimplemented
                // let rpath = result.as_ref().map_or_else(
                //     |(_, target)| target.check_rpath(),
                //     |(_, block, _)| block.rpath(),
                // );
                // let client = self
                //     .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.rpath())
                //     .await?;
                // client
                //     .on_message(
                //         HotstuffMessage::ProposalResult(
                //             proposal_id,
                //             result.map_err(|(err, _)| err),
                //         ),
                //         remote,
                //     )
                //     .await;
            }
            HotstuffPackage::QueryState(target, sub_path) => {
                let rpath = target.check_rpath();
                let service = self
                    .find_rpath_service_inner(
                        rpath.group_id(),
                        rpath.dec_id(),
                        rpath.rpath(),
                        false,
                        None,
                        Some(&remote),
                    )
                    .await;

                match service {
                    Ok(service) => {
                        service
                            .on_message(HotstuffMessage::QueryState(sub_path), remote)
                            .await;
                    }
                    _ => {
                        let client = self
                            .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.rpath())
                            .await?;
                        client
                            .on_message(HotstuffMessage::QueryState(sub_path), remote)
                            .await;
                    }
                }
            }
            HotstuffPackage::VerifiableState(sub_path, result) => {
                let rpath = result.as_ref().map_or_else(
                    |(_, target)| target.check_rpath(),
                    |status| status.block_desc.content().rpath(),
                );
                let client = self
                    .rpath_client(rpath.group_id(), rpath.dec_id(), rpath.rpath())
                    .await?;
                client
                    .on_message(
                        HotstuffMessage::VerifiableState(sub_path, result.map_err(|(err, _)| err)),
                        remote,
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn find_rpath_service_inner(
        &self,
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        is_auto_create: bool,
        block: Option<&GroupConsensusBlock>,
        remote: Option<&ObjectId>,
    ) -> BuckyResult<RPathService> {
        {
            // read
            let raw = self.read().await;
            let found = raw
                .service_by_group
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
            let local_id = local_info.bdt_stack.local_const().owner().unwrap();
            let local_device_id = local_info.bdt_stack.local_device_id();
            let signer = local_info.signer.clone();
            let non_driver = NONDriverHelper::new(
                local_info.non_driver.clone(),
                dec_id.clone(),
                local_device_id.object_id().clone(),
            );
            let root_state_mgr = local_info.global_state_mgr.clone();
            let network_sender = crate::network::Sender::new(
                local_info.datagram.clone(),
                non_driver.clone(),
                local_device_id.object_id().clone(),
            );
            let local_device_id = local_info.bdt_stack.local_device_id().clone();

            let shell_mgr = self
                .check_group_shell_mgr(group_id, non_driver.clone(), remote)
                .await?;

            let store = GroupStorage::load(
                group_id,
                dec_id,
                rpath,
                non_driver.clone(),
                local_device_id.object_id().clone(),
                &root_state_mgr,
            )
            .await;
            let store = match store {
                Ok(store) => Some(store),
                Err(e) => {
                    if let BuckyErrorCode::NotFound = e.code() {
                        log::warn!("{}/{}/{} not found in storage", group_id, dec_id, rpath);

                        self.on_new_rpath_request(
                            group_id.clone(),
                            dec_id,
                            rpath.to_string(),
                            block.cloned(),
                        )
                        .await?;

                        if !is_auto_create {
                            return Err(e);
                        } else {
                            None
                        }
                    } else {
                        return Err(e);
                    }
                }
            };

            let mut raw = self.write().await;

            let store = match store {
                Some(store) => store,
                None => {
                    GroupStorage::create(
                        group_id,
                        dec_id,
                        rpath,
                        non_driver.clone(),
                        local_device_id.object_id().clone(),
                        &root_state_mgr,
                    )
                    .await?
                }
            };

            let found = raw
                .service_by_group
                .entry(group_id.clone())
                .or_insert_with(HashMap::new)
                .entry(dec_id.clone())
                .or_insert_with(HashMap::new)
                .entry(rpath.to_string());

            match found {
                std::collections::hash_map::Entry::Occupied(found) => Ok(found.get().clone()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let service = RPathService::load(
                        local_id,
                        local_device_id.object_id().clone(),
                        GroupRPath::new(group_id.clone(), dec_id.clone(), rpath.to_string()),
                        signer,
                        RPathEventNotifier::new(non_driver.clone()),
                        network_sender,
                        non_driver,
                        shell_mgr,
                        store,
                    )
                    .await?;
                    entry.insert(service.clone());
                    Ok(service)
                }
            }
        }
    }

    async fn on_new_rpath_request(
        &self,
        group_id: ObjectId,
        dec_id: &ObjectId,
        rpath: String,
        with_block: Option<GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        let cmd = GroupCommandNewRPath {
            group_id,
            rpath,
            with_block,
        };

        let cmd = GroupCommand::from(cmd);
        let object_raw_buf = cmd.to_vec()?;
        let any_obj = cyfs_base::AnyNamedObject::Core(TypelessCoreObject::clone_from_slice(
            object_raw_buf.as_slice(),
        )?);
        let result = self
            .local_info()
            .non_driver
            .post_object(
                dec_id,
                NONObjectInfo {
                    object_id: cmd.desc().object_id(),
                    object_raw: object_raw_buf,
                    object: Some(Arc::new(any_obj)),
                },
                None,
            )
            .await?;

        assert!(result.is_none());
        Ok(())
    }

    async fn check_group_shell_mgr(
        &self,
        group_id: &ObjectId,
        non_driver: NONDriverHelper,
        remote: Option<&ObjectId>,
    ) -> BuckyResult<GroupShellManager> {
        {
            let raw = self.read().await;
            if let Some(shell_mgr) = raw.shell_mgr.get(group_id) {
                return Ok(shell_mgr.clone());
            }
        }

        let local_info = self.local_info();
        let ret = GroupShellManager::load(
            group_id,
            non_driver.clone(),
            local_info.meta_client.clone(),
            local_info.bdt_stack.local_device_id().object_id().clone(),
            &local_info.global_state_mgr.clone(),
        )
        .await;

        let shell_mgr = match ret {
            Ok(shell_mgr) => shell_mgr,
            Err(err) => {
                if err.code() == BuckyErrorCode::NotFound {
                    log::debug!(
                        "shells of group({}) not found, will create automatically.",
                        group_id
                    );
                    GroupShellManager::create(
                        group_id,
                        non_driver.clone(),
                        local_info.meta_client.clone(),
                        local_info.bdt_stack.local_device_id().object_id().clone(),
                        &local_info.global_state_mgr.clone(),
                        remote,
                    )
                    .await?
                } else {
                    log::error!("load shells of group({}) failed, err {:?}", group_id, err);
                    return Err(err);
                }
            }
        };

        {
            let mut raw = self.write().await;
            let shell_mgr = match raw.shell_mgr.get(group_id) {
                Some(shell_mgr) => shell_mgr.clone(),
                None => {
                    raw.shell_mgr.insert(group_id.clone(), shell_mgr.clone());
                    shell_mgr
                }
            };

            Ok(shell_mgr)
        }
    }
}
