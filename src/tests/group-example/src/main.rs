use std::{clone, time::Duration};

use cyfs_base::{NamedObject, ObjectDesc, ObjectId};
use cyfs_core::{GroupProposal, GroupProposalObject, GroupRPath};
use cyfs_group::IsCreateRPath;
use Common::{
    create_stack, EXAMPLE_ADMINS, EXAMPLE_APP_NAME, EXAMPLE_DEC_APP_ID, EXAMPLE_GROUP,
    EXAMPLE_RPATH,
};
use GroupDecService::DecService;

mod Common {
    use std::sync::Arc;

    use cyfs_base::{
        AnyNamedObject, Area, Device, DeviceCategory, DeviceId, Endpoint, Group, GroupMember,
        NamedObject, ObjectDesc, People, PrivateKey, Protocol, RawConvertTo, RawFrom,
        StandardObject, TypelessCoreObject, UniqueId,
    };
    use cyfs_core::{DecApp, DecAppId, DecAppObj};
    use cyfs_lib::{BrowserSanboxMode, NONObjectInfo};
    use cyfs_meta_lib::MetaMinerTarget;
    use cyfs_stack::{
        BdtStackParams, CyfsStack, CyfsStackConfigParams, CyfsStackFrontParams,
        CyfsStackInterfaceParams, CyfsStackKnownObjects, CyfsStackKnownObjectsInitMode,
        CyfsStackMetaParams, CyfsStackNOCParams, CyfsStackParams,
    };
    use rand::Rng;

    lazy_static::lazy_static! {
        pub static ref EXAMPLE_ADMINS: Vec<(People, PrivateKey, Device)> = create_members("admin", 4);
        pub static ref EXAMPLE_MEMBERS: Vec<(People, PrivateKey, Device)> = create_members("member", 9);

        pub static ref EXAMPLE_GROUP: Group = create_group(&EXAMPLE_ADMINS.get(0).unwrap().0, EXAMPLE_ADMINS.iter().map(|m| &m.0).collect(), EXAMPLE_MEMBERS.iter().map(|m| &m.0).collect(), EXAMPLE_ADMINS.iter().map(|m| &m.2).collect());
        pub static ref EXAMPLE_APP_NAME: String = "group-example".to_string();
        pub static ref EXAMPLE_DEC_APP: DecApp = DecApp::create(EXAMPLE_ADMINS.get(0).unwrap().0.desc().object_id(), EXAMPLE_APP_NAME.as_str());
        pub static ref EXAMPLE_DEC_APP_ID: DecAppId = DecAppId::try_from(EXAMPLE_DEC_APP.desc().object_id()).unwrap();
        pub static ref EXAMPLE_RPATH: String = "rpath-example".to_string();
    }

    fn create_members(name_prefix: &str, count: usize) -> Vec<(People, PrivateKey, Device)> {
        let port_begin = rand::thread_rng().gen_range(30000u16..60000u16);
        let mut members = vec![];

        for i in 0..count {
            let name = format!("{}-{}", name_prefix, i);
            let private_key = PrivateKey::generate_secp256k1().unwrap();
            let mut owner =
                People::new(None, vec![], private_key.public(), None, Some(name), None).build();

            let mut endpoint = Endpoint::default();
            endpoint.set_protocol(Protocol::Udp);
            endpoint.mut_addr().set_port(port_begin + i as u16);
            endpoint.set_static_wan(true);

            let device = Device::new(
                Some(owner.desc().object_id()),
                UniqueId::create_with_random(),
                vec![endpoint],
                vec![], // TODO: 当前版本是否支持无SN？
                vec![],
                private_key.public(),
                Area::default(),
                DeviceCategory::PC,
            )
            .build();

            owner
                .ood_list_mut()
                .push(DeviceId::try_from(device.desc().object_id()).unwrap());
            members.push((owner, private_key, device));
        }

        members
    }

    fn create_group(
        founder: &People,
        admins: Vec<&People>,
        members: Vec<&People>,
        oods: Vec<&Device>,
    ) -> Group {
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
        group
    }

    pub async fn create_stack(
        people: People,
        private_key: PrivateKey,
        device: Device,
    ) -> CyfsStack {
        let mut admin_device: Vec<Device> = EXAMPLE_ADMINS.iter().map(|m| m.2.clone()).collect();
        let mut member_device: Vec<Device> = EXAMPLE_MEMBERS.iter().map(|m| m.2.clone()).collect();
        let known_device = vec![admin_device, member_device].concat();

        let bdt_param = BdtStackParams {
            device,
            tcp_port_mapping: vec![],
            secret: private_key,
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

        for (member, _, device) in EXAMPLE_ADMINS.iter() {
            known_objects.list.push(NONObjectInfo::new(
                member.desc().object_id(),
                member.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::People(
                    member.clone(),
                )))),
            ));

            known_objects.list.push(NONObjectInfo::new(
                member.desc().object_id(),
                member.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                    device.clone(),
                )))),
            ));
        }

        for (member, _, device) in EXAMPLE_MEMBERS.iter() {
            known_objects.list.push(NONObjectInfo::new(
                member.desc().object_id(),
                member.to_vec().unwrap(),
                Some(Arc::new(AnyNamedObject::Standard(StandardObject::People(
                    member.clone(),
                )))),
            ));

            known_objects.list.push(NONObjectInfo::new(
                member.desc().object_id(),
                member.to_vec().unwrap(),
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

        CyfsStack::open(bdt_param, stack_param, known_objects)
            .await
            .unwrap()
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
    }

    impl MyRPathDelegate {
        pub fn new(local_name: String) -> Self {
            MyRPathDelegate { local_name }
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
                    let buf = pre_state_id.get_slice_value();
                    let mut pre_value = [0u8; 8];
                    pre_value.copy_from_slice(&buf[..8]);
                    u64::from_be_bytes(pre_value)
                });

                let delta_buf = proposal.params().as_ref().unwrap().as_slice();
                let mut delta = [0u8; 8];
                delta.copy_from_slice(delta_buf);
                let delta = u64::from_be_bytes(delta);

                let value = pre_value + delta;
                ObjectId::from_slice_value(&value.to_be_bytes())
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
                let buf = pre_state_id.get_slice_value();
                let mut pre_value = [0u8; 8];
                pre_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(pre_value)
            });

            let result_value = execute_result.result_state_id.map_or(0, |result_id| {
                let buf = result_id.get_slice_value();
                let mut result_value = [0u8; 8];
                result_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(result_value)
            });

            log::info!(
                "proposal commited: height: {}, delta: {}, result: {} -> {}, local: {}",
                block.height(),
                delta,
                pre_value,
                result_value,
                self.local_name
            );
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

    let mut admin_stacks = vec![];
    for (admin, private_key, device) in EXAMPLE_ADMINS.iter() {
        let cyfs_stack = create_stack(admin.clone(), private_key.clone(), device.clone()).await;
        DecService::run(&cyfs_stack, admin.name().unwrap().to_string()).await;
        admin_stacks.push(cyfs_stack);
    }

    for i in 1..100000000 {
        let stack = admin_stacks.get(i % admin_stacks.len()).unwrap();
        let owner = &EXAMPLE_ADMINS.get(i % EXAMPLE_ADMINS.len()).unwrap().0;
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

        async_std::task::spawn(async move {
            control.push_proposal(proposal).await.unwrap();
        });

        async_std::task::sleep(Duration::from_millis(1000)).await;
    }
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
