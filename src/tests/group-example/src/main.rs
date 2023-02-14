use std::{clone, sync::Arc, time::Duration};

use cyfs_base::{
    AnyNamedObject, NamedObject, ObjectDesc, ObjectId, RawConvertTo, RawFrom, TypelessCoreObject,
};
use cyfs_core::{GroupProposal, GroupProposalObject, GroupRPath};
use cyfs_group::IsCreateRPath;
use cyfs_lib::{
    DeviceZoneCategory, DeviceZoneInfo, NONObjectInfo, NamedObjectCachePutObjectRequest,
    NamedObjectStorageCategory, RequestProtocol, RequestSourceInfo,
};
use cyfs_stack::CyfsStack;
use Common::{
    create_stack, dummy, EXAMPLE_ADMINS, EXAMPLE_APP_NAME, EXAMPLE_DEC_APP_ID, EXAMPLE_GROUP,
    EXAMPLE_RPATH,
};
use GroupDecService::DecService;

mod Common {
    use std::sync::Arc;

    use cyfs_base::{
        AnyNamedObject, Area, Device, DeviceCategory, DeviceId, Endpoint, EndpointArea, Group,
        GroupMember, IpAddr, NamedObject, ObjectDesc, People, PrivateKey, Protocol, RawConvertTo,
        RawEncode, RawFrom, RsaCPUObjectSigner, Signer, StandardObject, TypelessCoreObject,
        UniqueId, SIGNATURE_SOURCE_REFINDEX_OWNER, SIGNATURE_SOURCE_REFINDEX_SELF,
    };
    use cyfs_bdt_ext::BdtStackParams;
    use cyfs_chunk_lib::ChunkMeta;
    use cyfs_core::{DecApp, DecAppId, DecAppObj};
    use cyfs_lib::{BrowserSanboxMode, NONObjectInfo};
    use cyfs_meta_lib::MetaMinerTarget;
    use cyfs_stack::{
        CyfsStack, CyfsStackConfigParams, CyfsStackFrontParams, CyfsStackInterfaceParams,
        CyfsStackKnownObjects, CyfsStackKnownObjectsInitMode, CyfsStackMetaParams,
        CyfsStackNOCParams, CyfsStackParams,
    };
    use rand::Rng;

    lazy_static::lazy_static! {
        pub static ref EXAMPLE_ADMINS: Vec<((People, PrivateKey), (Device, PrivateKey))> = create_members("admin", 4);
        pub static ref EXAMPLE_MEMBERS: Vec<((People, PrivateKey), (Device, PrivateKey))> = create_members("member", 9);

        pub static ref EXAMPLE_GROUP: Group = create_group(&EXAMPLE_ADMINS.get(0).unwrap().0.0, EXAMPLE_ADMINS.iter().map(|m| &m.0.0).collect(), EXAMPLE_MEMBERS.iter().map(|m| &m.0.0).collect(), EXAMPLE_ADMINS.iter().map(|m| &m.1.0).collect());
        pub static ref EXAMPLE_GROUP_CHUNK: ChunkMeta = ChunkMeta::from(&*EXAMPLE_GROUP);
        pub static ref EXAMPLE_APP_NAME: String = "group-example".to_string();
        pub static ref EXAMPLE_DEC_APP: DecApp = DecApp::create(EXAMPLE_ADMINS.get(0).unwrap().0.0.desc().object_id(), EXAMPLE_APP_NAME.as_str());
        pub static ref EXAMPLE_DEC_APP_ID: DecAppId = DecAppId::try_from(EXAMPLE_DEC_APP.desc().object_id()).unwrap();
        pub static ref EXAMPLE_RPATH: String = "rpath-example".to_string();
    }

    fn create_members(
        name_prefix: &str,
        count: usize,
    ) -> Vec<((People, PrivateKey), (Device, PrivateKey))> {
        log::info!("create members");

        let port_begin = rand::thread_rng().gen_range(30000u16..60000u16);
        let mut members = vec![];

        for i in 0..count {
            let name = format!("{}-{}", name_prefix, i);
            let private_key = PrivateKey::generate_rsa(1024).unwrap();
            let device_private_key = PrivateKey::generate_rsa(1024).unwrap();
            let mut owner =
                People::new(None, vec![], private_key.public(), None, Some(name), None).build();

            let mut endpoint = Endpoint::default();
            endpoint.set_protocol(Protocol::Udp);
            endpoint
                .mut_addr()
                .set_ip(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 100, 120)));
            endpoint.mut_addr().set_port(port_begin + i as u16);
            endpoint.set_area(EndpointArea::Wan);

            let mut device = Device::new(
                Some(owner.desc().object_id()),
                UniqueId::create_with_random(),
                vec![endpoint],
                vec![], // TODO: 当前版本是否支持无SN？
                vec![],
                device_private_key.public(),
                Area::default(),
                DeviceCategory::PC,
            )
            .build();

            owner
                .ood_list_mut()
                .push(DeviceId::try_from(device.desc().object_id()).unwrap());

            let signer = RsaCPUObjectSigner::new(private_key.public(), private_key.clone());

            let owner_desc_hash = owner.desc().raw_hash_value().unwrap();
            let owner_body_hash = owner.body().as_ref().unwrap().raw_hash_value().unwrap();
            let device_desc_hash = device.desc().raw_hash_value().unwrap();
            let device_body_hash = device.body().as_ref().unwrap().raw_hash_value().unwrap();

            let (owner_desc_signature, owner_body_signature, desc_signature, body_signature) =
                async_std::task::block_on(async move {
                    let owner_desc_signature = signer
                        .sign(
                            owner_desc_hash.as_slice(),
                            &cyfs_base::SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
                        )
                        .await
                        .unwrap();

                    let owner_body_signature = signer
                        .sign(
                            owner_body_hash.as_slice(),
                            &cyfs_base::SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
                        )
                        .await
                        .unwrap();

                    let desc_signature = signer
                        .sign(
                            device_desc_hash.as_slice(),
                            &cyfs_base::SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
                        )
                        .await
                        .unwrap();

                    let body_signature = signer
                        .sign(
                            device_body_hash.as_slice(),
                            &cyfs_base::SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
                        )
                        .await
                        .unwrap();

                    (
                        owner_desc_signature,
                        owner_body_signature,
                        desc_signature,
                        body_signature,
                    )
                });

            device.signs_mut().set_desc_sign(desc_signature.clone());
            device.signs_mut().set_body_sign(body_signature);

            log::info!(
                "people: {:?}/{:?}, device: {:?}, public-key: {:?}, private-key: {:?}, sign: {:?}, object: {:?}",
                owner.name().unwrap(),
                owner.desc().object_id(),
                device.desc().object_id(),
                private_key.public().to_hex().unwrap().split_at(32).0,
                private_key.to_string(),
                desc_signature.to_hex().unwrap(),
                owner.body().as_ref().unwrap().raw_hash_value().unwrap().to_hex()
            );

            owner.signs_mut().set_desc_sign(owner_desc_signature);
            owner.signs_mut().set_body_sign(owner_body_signature);
            members.push(((owner, private_key), (device, device_private_key)));
        }

        members
    }

    fn create_group(
        founder: &People,
        admins: Vec<&People>,
        members: Vec<&People>,
        oods: Vec<&Device>,
    ) -> Group {
        log::info!("create group");

        let mut group = Group::new_org(founder.desc().object_id(), Area::default()).build();
        group.check_org_body_content_mut().set_admins(
            admins
                .iter()
                .map(|m| GroupMember::from_member_id(m.desc().object_id()))
                .collect(),
        );
        group.set_members(
            members
                .iter()
                .map(|m| GroupMember::from_member_id(m.desc().object_id()))
                .collect(),
        );
        group.set_ood_list(
            oods.iter()
                .map(|d| DeviceId::try_from(d.desc().object_id()).unwrap())
                .collect(),
        );

        group.set_consensus_interval(5000);

        log::info!("create group: {:?}", group.desc().object_id());

        group
    }

    fn init_stack_params(
        people: People,
        private_key: &PrivateKey,
        device: Device,
    ) -> Box<(BdtStackParams, CyfsStackParams, CyfsStackKnownObjects)> {
        log::info!("init_stack_params");

        let mut admin_device: Vec<Device> = EXAMPLE_ADMINS.iter().map(|m| m.1 .0.clone()).collect();
        let mut member_device: Vec<Device> =
            EXAMPLE_MEMBERS.iter().map(|m| m.1 .0.clone()).collect();
        let known_device = vec![admin_device, member_device].concat();

        let bdt_param = BdtStackParams {
            device: device.clone(),
            tcp_port_mapping: vec![],
            secret: private_key.clone(),
            known_sn: vec![],
            known_device,
            known_passive_pn: vec![],
            udp_sn_only: None,
        };

        let stack_param = CyfsStackParams {
            config: CyfsStackConfigParams {
                isolate: None,
                sync_service: false,
                shared_stack: false,
            },
            noc: CyfsStackNOCParams {},
            interface: CyfsStackInterfaceParams {
                bdt_listeners: vec![],
                tcp_listeners: vec![],
                ws_listener: None,
            },
            meta: CyfsStackMetaParams {
                target: MetaMinerTarget::Dev,
            },
            front: CyfsStackFrontParams {
                enable: false,
                browser_mode: BrowserSanboxMode::None,
            },
        };

        let mut known_objects = CyfsStackKnownObjects {
            list: vec![],
            mode: CyfsStackKnownObjectsInitMode::Sync,
        };

        for ((member, _), (device, _)) in EXAMPLE_ADMINS.iter() {
            known_objects.list.push(NONObjectInfo::new(
                member.desc().object_id(),
                member.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::People(
                    member.clone(),
                )))),
            ));

            known_objects.list.push(NONObjectInfo::new(
                device.desc().object_id(),
                device.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                    device.clone(),
                )))),
            ));
        }

        for ((member, _), (device, _)) in EXAMPLE_MEMBERS.iter() {
            known_objects.list.push(NONObjectInfo::new(
                member.desc().object_id(),
                member.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::People(
                    member.clone(),
                )))),
            ));

            known_objects.list.push(NONObjectInfo::new(
                device.desc().object_id(),
                device.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                    device.clone(),
                )))),
            ));
        }

        known_objects.list.push(NONObjectInfo::new(
            EXAMPLE_GROUP.desc().object_id(),
            EXAMPLE_GROUP.to_vec().unwrap(),
            Some(Arc::new(AnyNamedObject::Standard(StandardObject::Group(
                EXAMPLE_GROUP.clone(),
            )))),
        ));

        let dec_app_vec = EXAMPLE_DEC_APP.to_vec().unwrap();
        let typeless = TypelessCoreObject::clone_from_slice(dec_app_vec.as_slice()).unwrap();
        known_objects.list.push(NONObjectInfo::new(
            EXAMPLE_DEC_APP.desc().object_id(),
            dec_app_vec,
            Some(Arc::new(AnyNamedObject::Core(typeless))),
        ));

        Box::new((bdt_param, stack_param, known_objects))
    }

    pub async fn create_stack(
        people: People,
        private_key: &PrivateKey,
        device: Device,
    ) -> CyfsStack {
        let params = init_stack_params(people, private_key, device);

        log::info!("cyfs-stack.open");

        let stack = CyfsStack::open(params.0, params.1, params.2)
            .await
            .map_err(|e| {
                log::error!("stack start failed: {}", e);
                e
            })
            .unwrap();

        stack
    }

    pub fn dummy(people: People, device: Device) {
        log::info!("common::dummy");
    }
}

mod Client {
    // use cyfs_base::ObjectId;
    // use cyfs_core::GroupProposal;
    // use cyfs_group::RPathClient;

    // pub struct DecClient {}

    // impl DecClient {
    //     async fn do_something(&self) {
    //         let rpath_client = RPathClient::new();

    //         let field_path = "/xxx/yyy";
    //         let old_value = rpath_client.get_field(field_path).await;
    //         let param = ObjectId::default(); // param = old_value.value
    //         let proposal = self.make_proposal(param);
    //         rpath_client.post_proposal(proposal).await;
    //     }

    //     fn make_proposal(&self, param: ObjectId) -> GroupProposal {
    //         unimplemented!()
    //     }
    // }
}

mod GroupDecService {
    use std::{collections::HashSet, sync::Arc};

    use async_std::sync::Mutex;
    use cyfs_base::*;
    use cyfs_core::{
        GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupProposalObject,
    };
    use cyfs_group::{DelegateFactory, ExecuteResult, RPathDelegate};
    use cyfs_stack::CyfsStack;

    use crate::Common::EXAMPLE_DEC_APP_ID;

    pub struct DecService {}

    impl DecService {
        pub async fn run(cyfs_stack: &CyfsStack, local_name: String) {
            let group_mgr = cyfs_stack.group_mgr();

            group_mgr
                .register(
                    EXAMPLE_DEC_APP_ID.clone(),
                    Box::new(GroupRPathDelegateFactory { local_name }),
                )
                .await
                .unwrap()
        }
    }

    pub struct GroupRPathDelegateFactory {
        local_name: String,
    }

    impl GroupRPathDelegateFactory {
        pub fn is_accept(
            &self,
            group: &Group,
            rpath: &str,
            with_block: Option<&GroupConsensusBlock>,
        ) -> bool {
            // 由应用定义是否接收该rpath，并启动共识过程，参与该rpath的信息维护
            true
        }
    }

    #[async_trait::async_trait]
    impl DelegateFactory for GroupRPathDelegateFactory {
        async fn create_rpath_delegate(
            &self,
            group: &Group,
            rpath: &str,
            with_block: Option<&GroupConsensusBlock>,
        ) -> BuckyResult<Box<dyn RPathDelegate>> {
            if self.is_accept(group, rpath, with_block) {
                // 如果接受，就提供该rpath的处理响应对象
                Ok(Box::new(MyRPathDelegate::new(self.local_name.clone())))
            } else {
                Err(BuckyError::new(BuckyErrorCode::Reject, ""))
            }
        }

        async fn on_state_changed(
            &self,
            group_id: &ObjectId,
            rpath: &str,
            state_id: Option<ObjectId>,
            pre_state_id: Option<ObjectId>,
        ) {
            unimplemented!()
        }
    }

    pub struct MyRPathDelegate {
        local_name: String,
        finished_proposals: Arc<Mutex<HashSet<ObjectId>>>,
    }

    impl MyRPathDelegate {
        pub fn new(local_name: String) -> Self {
            MyRPathDelegate {
                local_name,
                finished_proposals: Arc::new(Mutex::new(HashSet::new())),
            }
        }
    }

    impl MyRPathDelegate {
        pub fn execute(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
        ) -> BuckyResult<ExecuteResult> {
            let result_state_id = {
                /**
                 * pre_state_id是一个MAP的操作对象，形式待定，可能就是一个SingleOpEnv，但最好支持多级路径操作
                 */
                let pre_value = pre_state_id.map_or(0, |pre_state_id| {
                    let buf = pre_state_id.data();
                    let mut pre_value = [0u8; 8];
                    pre_value.copy_from_slice(&buf[..8]);
                    u64::from_be_bytes(pre_value)
                });

                let delta_buf = proposal.params().as_ref().unwrap().as_slice();
                let mut delta = [0u8; 8];
                delta.copy_from_slice(delta_buf);
                let delta = u64::from_be_bytes(delta);

                let value = pre_value + delta;
                ObjectIdDataBuilder::new()
                    .data(&value.to_be_bytes())
                    .build()
                    .unwrap()
            };

            let receipt = {
                /**
                 * 返回给Client的对象，相当于这个请求的结果或者叫回执？
                 */
                None
            };

            let context = {
                /**
                 * 执行请求的上下文，运算过程中可能有验证节点无法得到的上下文信息（比如时间戳，随机数）
                 */
                None
            };

            /**
             * (result_state_id, return_object) = pre_state_id + proposal + context
             */
            Ok(ExecuteResult {
                context,
                result_state_id: Some(result_state_id),
                receipt,
            })
        }

        pub fn verify(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<bool> {
            /**
             * let is_same = (execute_result.result_state_id, execute_result.return_object)
             *  == pre_state_id + proposal + execute_result.context
             */
            let result = self.execute(proposal, pre_state_id)?;

            let is_ok = execute_result.result_state_id == result.result_state_id
                && execute_result.context.is_none()
                && execute_result.receipt.is_none();

            Ok(is_ok)
        }
    }

    #[async_trait::async_trait]
    impl RPathDelegate for MyRPathDelegate {
        async fn on_execute(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
        ) -> BuckyResult<ExecuteResult> {
            self.execute(proposal, pre_state_id)
        }

        async fn on_verify(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<bool> {
            self.verify(proposal, pre_state_id, execute_result)
        }

        async fn on_commited(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
            block: &GroupConsensusBlock,
        ) {
            // 提交到共识链上了，可能有些善后事宜

            let delta_buf = proposal.params().as_ref().unwrap().as_slice();
            let mut delta = [0u8; 8];
            delta.copy_from_slice(delta_buf);
            let delta = u64::from_be_bytes(delta);

            let pre_value = pre_state_id.map_or(0, |pre_state_id| {
                let buf = pre_state_id.data();
                let mut pre_value = [0u8; 8];
                pre_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(pre_value)
            });

            let result_value = execute_result.result_state_id.map_or(0, |result_id| {
                let buf = result_id.data();
                let mut result_value = [0u8; 8];
                result_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(result_value)
            });

            let proposal_id = proposal.desc().object_id();

            log::info!(
                "proposal commited: height: {}/{}, delta: {}, result: {} -> {}, proposal: {}, block: {}, local: {}",
                block.height(), block.round(),
                delta,
                pre_value,
                result_value,
                proposal_id,
                block.block_id(),
                self.local_name
            );

            let is_new_finished = self.finished_proposals.lock().await.insert(proposal_id);
            assert!(is_new_finished);
        }

        async fn get_group(&self, group_chunk_id: Option<&ObjectId>) -> BuckyResult<Group> {
            unimplemented!()
        }
    }
}

fn create_proposal(delta: u64, owner: ObjectId) -> GroupProposal {
    GroupProposal::create(
        GroupRPath::new(
            EXAMPLE_GROUP.desc().object_id(),
            EXAMPLE_DEC_APP_ID.object_id().clone(),
            EXAMPLE_RPATH.clone(),
        ),
        "add".to_string(),
        Some(Vec::from(delta.to_be_bytes())),
        None,
        None,
        owner,
        None,
        None,
        None,
    )
    .build()
}

async fn main_run() {
    log::info!("main_run");

    cyfs_debug::CyfsLoggerBuilder::new_app(EXAMPLE_APP_NAME.as_str())
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new(EXAMPLE_APP_NAME.as_str(), EXAMPLE_APP_NAME.as_str())
        .exit_on_panic(true)
        .build()
        .start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    log::info!("will open stacks");

    let mut admin_stacks: Vec<CyfsStack> = vec![];
    for ((admin, _), (device, private_key)) in EXAMPLE_ADMINS.iter() {
        // dummy(admin.clone(), device.clone());
        let cyfs_stack = create_stack(admin.clone(), private_key, device.clone()).await;
        admin_stacks.push(cyfs_stack);
    }

    for i in 0..admin_stacks.len() {
        let stack = admin_stacks.get(i).unwrap();
        let ((admin, _), _) = EXAMPLE_ADMINS.get(i).unwrap();
        DecService::run(&stack, admin.name().unwrap().to_string()).await;

        let control = stack
            .group_mgr()
            .find_rpath_control(
                &EXAMPLE_GROUP.desc().object_id(),
                EXAMPLE_DEC_APP_ID.object_id(),
                &EXAMPLE_RPATH,
                IsCreateRPath::Yes(None),
            )
            .await
            .unwrap();
    }

    async_std::task::sleep(Duration::from_millis(30000)).await;

    let mut proposals: Vec<GroupProposal> = vec![];

    log::info!("proposals will be prepared.");

    let PROPOSAL_COUNT = 1000usize;
    for i in 1..PROPOSAL_COUNT {
        let stack = admin_stacks.get(i % admin_stacks.len()).unwrap();
        let owner = &EXAMPLE_ADMINS.get(i % EXAMPLE_ADMINS.len()).unwrap().0 .0;
        let proposal = create_proposal(i as u64, owner.desc().object_id());

        let control = stack
            .group_mgr()
            .find_rpath_control(
                &EXAMPLE_GROUP.desc().object_id(),
                EXAMPLE_DEC_APP_ID.object_id(),
                &EXAMPLE_RPATH,
                IsCreateRPath::Yes(None),
            )
            .await
            .unwrap();

        let noc = stack.noc_manager().clone();

        let buf = proposal.to_vec().unwrap();
        let proposal_any = Arc::new(AnyNamedObject::Core(
            TypelessCoreObject::clone_from_slice(buf.as_slice()).unwrap(),
        ));

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo {
                protocol: RequestProtocol::DatagramBdt,
                zone: DeviceZoneInfo {
                    device: None,
                    zone: None,
                    zone_category: DeviceZoneCategory::CurrentDevice,
                },
                dec: EXAMPLE_DEC_APP_ID.object_id().clone(),
                verified: None,
            },
            object: NONObjectInfo::new(proposal.desc().object_id(), buf, Some(proposal_any)),
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };
        noc.put_object(&req).await;
        proposals.push(proposal);
    }

    // futures::future::join_all(prepare_futures).await;

    log::info!("proposals prepared.");

    for i in 0..(PROPOSAL_COUNT - 1) {
        let proposal = proposals.get(i).unwrap().clone();
        let stack = admin_stacks.get(i % admin_stacks.len()).unwrap();

        let control = stack
            .group_mgr()
            .find_rpath_control(
                &EXAMPLE_GROUP.desc().object_id(),
                EXAMPLE_DEC_APP_ID.object_id(),
                &EXAMPLE_RPATH,
                IsCreateRPath::Yes(None),
            )
            .await
            .unwrap();

        async_std::task::spawn(async move {
            control.push_proposal(proposal).await.unwrap();
        });

        if i % 10 == 0 {
            async_std::task::sleep(Duration::from_millis(200)).await;
            log::info!("will push new proposals, i: {}", i);
        }
    }
}

fn main() {
    log::info!("main");

    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    log::info!("will main-run");

    let fut = Box::pin(main_run());
    async_std::task::block_on(async move { fut.await })
}
