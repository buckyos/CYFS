use std::{clone, sync::Arc, time::Duration};

use cyfs_base::{
    AccessString, AnyNamedObject, NamedObject, ObjectDesc, ObjectId, RawConvertTo, RawFrom,
    TypelessCoreObject,
};
use cyfs_core::{DecApp, DecAppId, DecAppObj, GroupProposal, GroupProposalObject, GroupRPath};
use cyfs_group::IsCreateRPath;
use cyfs_lib::{
    DeviceZoneCategory, DeviceZoneInfo, NONObjectInfo, NamedObjectCachePutObjectRequest,
    NamedObjectStorageCategory, RequestProtocol, RequestSourceInfo,
};
use cyfs_stack::CyfsStack;
use Common::{create_stack, EXAMPLE_RPATH};
use GroupDecService::DecService;

use crate::Common::{init_admins, init_group, init_members, EXAMPLE_APP_NAME};

mod Common {
    use std::{fmt::format, io::ErrorKind, sync::Arc};

    use async_std::{fs, stream::StreamExt};
    use cyfs_base::{
        AnyNamedObject, Area, Device, DeviceCategory, DeviceId, Endpoint, EndpointArea, Group,
        GroupMember, IpAddr, NamedObject, ObjectDesc, People, PrivateKey, Protocol, RawConvertTo,
        RawDecode, RawEncode, RawFrom, RsaCPUObjectSigner, Signer, StandardObject,
        TypelessCoreObject, UniqueId, NON_STACK_BDT_VPORT, NON_STACK_SYNC_BDT_VPORT,
        SIGNATURE_SOURCE_REFINDEX_OWNER, SIGNATURE_SOURCE_REFINDEX_SELF,
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

    /**
     * |--root
     *      |--folder1
     *          |--folder2
     *              |--value-->u64
     */

    lazy_static::lazy_static! {
        pub static ref EXAMPLE_APP_NAME: String = "group-example".to_string();
        pub static ref EXAMPLE_RPATH: String = "rpath-example".to_string();
        pub static ref EXAMPLE_VALUE_PATH: String = "/root/folder1/folder2/value".to_string();
        pub static ref STATE_PATH_SEPARATOR: String = "/".to_string();
    }

    fn create_member(
        name_prefix: &str,
        index: usize,
        port: u16,
    ) -> ((People, PrivateKey), (Device, PrivateKey)) {
        log::info!("create members");

        let name = format!("{}-{}", name_prefix, index);
        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let device_private_key = PrivateKey::generate_rsa(1024).unwrap();
        let mut owner =
            People::new(None, vec![], private_key.public(), None, Some(name), None).build();

        let mut endpoint = Endpoint::default();
        endpoint.set_protocol(Protocol::Udp);
        endpoint
            .mut_addr()
            .set_ip(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 100, 120)));
        endpoint.mut_addr().set_port(port);
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
        ((owner, private_key), (device, device_private_key))
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

    // (succ, not-found)
    fn check_read_buf(file_path: &str, result: &std::io::Result<Vec<u8>>) -> (bool, bool) {
        match result.as_ref() {
            Ok(b) => {
                if b.len() == 0 {
                    (false, true)
                } else {
                    (true, false)
                }
            }
            Err(err) if ErrorKind::NotFound == err.kind() => (false, true),
            Err(err) => {
                log::warn!("read file {} failed: {:?}", file_path, err);
                (false, false)
            }
        }
    }

    async fn init_member_from_dir(
        save_path: &str,
        name_prefix: &str,
        count: usize,
        min_port: u16,
    ) -> Vec<((People, PrivateKey), (Device, PrivateKey))> {
        fs::create_dir_all(save_path)
            .await
            .expect(format!("create dir {} failed", save_path).as_str());

        let mut members = vec![];

        for i in 0..count {
            let index = i + 1;
            let people_desc_file_path = format!("{}/people-{}.desc", save_path, index);
            let people_sec_file_path = format!("{}/people-{}.sec", save_path, index);
            let device_desc_file_path = format!("{}/device-{}.desc", save_path, index);
            let device_sec_file_path = format!("{}/device-{}.sec", save_path, index);

            let people_desc_r = fs::read(people_desc_file_path.clone()).await;
            let people_sec_r = fs::read(people_sec_file_path.clone()).await;
            let device_desc_r = fs::read(device_desc_file_path.clone()).await;
            let device_sec_r = fs::read(device_sec_file_path.clone()).await;

            let (is_people_desc_succ, is_people_desc_not_found) =
                check_read_buf(people_desc_file_path.as_str(), &people_desc_r);
            let (is_people_sec_succ, is_people_sec_not_found) =
                check_read_buf(people_sec_file_path.as_str(), &people_sec_r);
            let (is_device_desc_succ, is_device_desc_not_found) =
                check_read_buf(device_desc_file_path.as_str(), &device_desc_r);
            let (is_device_sec_succ, is_device_sec_not_found) =
                check_read_buf(device_sec_file_path.as_str(), &device_sec_r);

            if is_people_desc_succ
                && is_people_sec_succ
                && is_device_desc_succ
                && is_device_sec_succ
            {
                // decode
                let people_desc = People::raw_decode(people_desc_r.unwrap().as_slice())
                    .expect(format!("decode file {} failed", people_desc_file_path).as_str())
                    .0;
                let people_sec = PrivateKey::raw_decode(people_sec_r.unwrap().as_slice())
                    .expect(format!("decode file {} failed", people_sec_file_path).as_str())
                    .0;
                let device_desc = Device::raw_decode(device_desc_r.unwrap().as_slice())
                    .expect(format!("decode file {} failed", device_desc_file_path).as_str())
                    .0;
                let device_sec = PrivateKey::raw_decode(device_sec_r.unwrap().as_slice())
                    .expect(format!("decode file {} failed", device_sec_file_path).as_str())
                    .0;
                members.push(((people_desc, people_sec), (device_desc, device_sec)));
            } else if is_people_desc_not_found
                && is_people_sec_not_found
                && is_device_desc_not_found
                && is_device_sec_not_found
            {
                // create & save
                let member = create_member(name_prefix, index, min_port + i as u16);
                fs::write(
                    people_desc_file_path.as_str(),
                    member.0 .0.to_vec().unwrap(),
                )
                .await
                .expect(format!("save file {} failed", people_desc_file_path).as_str());
                fs::write(people_sec_file_path.as_str(), member.0 .1.to_vec().unwrap())
                    .await
                    .expect(format!("save file {} failed", people_sec_file_path).as_str());
                fs::write(
                    device_desc_file_path.as_str(),
                    member.1 .0.to_vec().unwrap(),
                )
                .await
                .expect(format!("save file {} failed", device_desc_file_path).as_str());
                fs::write(device_sec_file_path.as_str(), member.1 .1.to_vec().unwrap())
                    .await
                    .expect(format!("save file {} failed", device_sec_file_path).as_str());

                members.push(member);
            } else {
                println!("read members failed!");
                std::process::exit(-1);
            }
        }

        members
    }

    pub async fn init_admins() -> Vec<((People, PrivateKey), (Device, PrivateKey))> {
        let min_port = 30217_u16;
        init_member_from_dir("./test-group/admins", "admin", 4, min_port).await
    }

    pub async fn init_members() -> Vec<((People, PrivateKey), (Device, PrivateKey))> {
        let min_port = 31217_u16;
        init_member_from_dir("./test-group/members", "member", 9, min_port).await
    }

    pub async fn init_group(
        admins: Vec<&People>,
        members: Vec<&People>,
        oods: Vec<&Device>,
    ) -> Group {
        fs::create_dir_all("./test-group")
            .await
            .expect("create dir ./test-group failed");

        let read_group_r = fs::read("./test-group/group.desc").await;
        match read_group_r {
            Ok(buf) => {
                if buf.len() > 0 {
                    return Group::raw_decode(buf.as_slice())
                        .expect("decode ./test-group/group.desc failed")
                        .0;
                }
            }
            Err(err) => {
                if ErrorKind::NotFound != err.kind() {
                    println!("read group failed: {:?}", err);
                    std::process::exit(-1);
                }
            }
        }

        let group = create_group(admins.get(0).unwrap(), admins, members, oods);
        fs::write("./test-group/group.desc", group.to_vec().unwrap())
            .await
            .expect("save file ./test-group/group.desc failed");
        group
    }

    fn init_stack_params(
        people: &People,
        private_key: &PrivateKey,
        device: &Device,
        admins: Vec<(People, Device)>,
        members: Vec<(People, Device)>,
        group: &Group,
        dec_app: &DecApp,
    ) -> Box<(BdtStackParams, CyfsStackParams, CyfsStackKnownObjects)> {
        log::info!("init_stack_params");

        let mut admin_device: Vec<Device> = admins.iter().map(|m| m.1.clone()).collect();
        let mut member_device: Vec<Device> = members.iter().map(|m| m.1.clone()).collect();
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
                isolate: Some(device.desc().object_id().to_string()),
                sync_service: false,
                shared_stack: false,
            },
            noc: CyfsStackNOCParams {},
            interface: CyfsStackInterfaceParams {
                bdt_listeners: vec![NON_STACK_BDT_VPORT, NON_STACK_SYNC_BDT_VPORT],
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

        for (member, device) in admins.iter() {
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

        for (member, device) in members.iter() {
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
            group.desc().object_id(),
            group.to_vec().unwrap(),
            Some(Arc::new(AnyNamedObject::Standard(StandardObject::Group(
                group.clone(),
            )))),
        ));

        let dec_app_vec = dec_app.to_vec().unwrap();
        let typeless = TypelessCoreObject::clone_from_slice(dec_app_vec.as_slice()).unwrap();
        known_objects.list.push(NONObjectInfo::new(
            dec_app.desc().object_id(),
            dec_app_vec,
            Some(Arc::new(AnyNamedObject::Core(typeless))),
        ));

        Box::new((bdt_param, stack_param, known_objects))
    }

    pub async fn create_stack(
        people: &People,
        private_key: &PrivateKey,
        device: &Device,
        admins: Vec<(People, Device)>,
        members: Vec<(People, Device)>,
        group: &Group,
        dec_app: &DecApp,
    ) -> CyfsStack {
        let params =
            init_stack_params(people, private_key, device, admins, members, group, dec_app);

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
    use std::{collections::HashSet, fmt::format, sync::Arc};

    use async_std::sync::Mutex;
    use cyfs_base::*;
    use cyfs_core::{
        DecAppId, GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal,
        GroupProposalObject,
    };
    use cyfs_group::{DelegateFactory, ExecuteResult, GroupObjectMapProcessor, RPathDelegate};
    use cyfs_stack::CyfsStack;

    use crate::Common::{EXAMPLE_VALUE_PATH, STATE_PATH_SEPARATOR};

    pub struct DecService {}

    impl DecService {
        pub async fn run(cyfs_stack: &CyfsStack, local_name: String, dec_app_id: DecAppId) {
            let group_mgr = cyfs_stack.group_mgr();

            group_mgr
                .register(
                    dec_app_id.clone(),
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
        pub async fn get_value_from_state_tree_with_single_op_envs(
            pre_state_id: Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) -> BuckyResult<(
            Option<ObjectId>,
            Vec<(ObjectMapSingleOpEnvRef, &str, Option<ObjectId>)>,
        )> {
            let mut single_op_envs = vec![];
            let mut parent_map_id = pre_state_id;
            for folder in EXAMPLE_VALUE_PATH.split(STATE_PATH_SEPARATOR.as_str()) {
                let single_op_env = object_map_processor.create_single_op_env().await.expect(
                    format!(
                        "create_single_op_env load folder {} with obj_id {:?} failed",
                        folder, parent_map_id
                    )
                    .as_str(),
                );
                parent_map_id = match parent_map_id {
                    Some(parent_map_id) => {
                        single_op_env.load(&parent_map_id).await.expect(
                            format!(
                                "load folder {} parent with obj_id {:?} failed",
                                folder, parent_map_id
                            )
                            .as_str(),
                        );
                        single_op_env.get_by_key(folder).await.expect(
                            format!(
                                "load folder {} with obj_id {:?} failed",
                                folder, parent_map_id
                            )
                            .as_str(),
                        )
                    }
                    None => {
                        single_op_env
                            .create_new(ObjectMapSimpleContentType::Map)
                            .await
                            .expect(
                                format!(
                                    "create folder {} with obj_id {:?} failed",
                                    folder, parent_map_id
                                )
                                .as_str(),
                            );
                        None
                    }
                };
                single_op_envs.push((single_op_env, folder, parent_map_id));
            }

            Ok((parent_map_id, single_op_envs))
        }

        pub async fn execute(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) -> BuckyResult<ExecuteResult> {
            let (pre_value, single_op_envs) = Self::get_value_from_state_tree_with_single_op_envs(
                pre_state_id,
                object_map_processor,
            )
            .await?;

            let result_value = {
                /**
                 * pre_state_id是一个MAP的操作对象，形式待定，可能就是一个SingleOpEnv，但最好支持多级路径操作
                 */
                let pre_value = pre_value.map_or(0, |pre_value| {
                    let buf = pre_value.data();
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

            let result_state_id = {
                let mut sub_folder_value = result_value;
                for (parent_single_op_env, folder, sub_folder_prev_value) in
                    single_op_envs.into_iter().rev()
                {
                    parent_single_op_env
                        .set_with_key(folder, &sub_folder_value, &sub_folder_prev_value, true)
                        .await
                        .expect(
                            format!(
                                "update folder {} value from {:?} to {:?} failed",
                                folder, sub_folder_prev_value, sub_folder_value
                            )
                            .as_str(),
                        );
                    sub_folder_value = parent_single_op_env.commit().await.expect(
                        format!(
                            "commit folder {} value from {:?} to {:?} failed",
                            folder, sub_folder_prev_value, sub_folder_value
                        )
                        .as_str(),
                    );
                }
                sub_folder_value
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
             * (result_state_id, return_object) = pre_value + proposal + context
             */
            Ok(ExecuteResult {
                context,
                result_state_id: Some(result_state_id),
                receipt,
            })
        }

        pub async fn verify(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<bool> {
            /**
             * let is_same = (execute_result.result_state_id, execute_result.return_object)
             *  == pre_state_id + proposal + execute_result.context
             */
            let result = self
                .execute(proposal, pre_state_id, object_map_processor)
                .await?;

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
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) -> BuckyResult<ExecuteResult> {
            self.execute(proposal, pre_state_id, object_map_processor)
                .await
        }

        async fn on_verify(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<bool> {
            self.verify(proposal, pre_state_id, object_map_processor, execute_result)
                .await
        }

        async fn on_commited(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
            execute_result: &ExecuteResult,
            block: &GroupConsensusBlock,
        ) {
            // 提交到共识链上了，可能有些善后事宜

            let delta_buf = proposal.params().as_ref().unwrap().as_slice();
            let mut delta = [0u8; 8];
            delta.copy_from_slice(delta_buf);
            let delta = u64::from_be_bytes(delta);

            let pre_value = Self::get_value_from_state_tree_with_single_op_envs(
                pre_state_id,
                object_map_processor,
            )
            .await
            .unwrap()
            .0
            .map_or(0, |pre_state_id| {
                let buf = pre_state_id.data();
                let mut pre_value = [0u8; 8];
                pre_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(pre_value)
            });

            let result_value = Self::get_value_from_state_tree_with_single_op_envs(
                execute_result.result_state_id,
                object_map_processor,
            )
            .await
            .unwrap()
            .0
            .map_or(0, |result_id| {
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

fn create_proposal(
    delta: u64,
    owner: ObjectId,
    group_id: ObjectId,
    dec_id: ObjectId,
) -> GroupProposal {
    GroupProposal::create(
        GroupRPath::new(group_id, dec_id, EXAMPLE_RPATH.to_string()),
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

    let admins = init_admins().await;
    let members = init_members().await;
    let group = init_group(
        admins.iter().map(|m| &m.0 .0).collect(),
        members.iter().map(|m| &m.0 .0).collect(),
        admins.iter().map(|m| &m.1 .0).collect(),
    )
    .await;
    let group_id = group.desc().object_id();
    let dec_app = DecApp::create(
        admins.get(0).unwrap().0 .0.desc().object_id(),
        EXAMPLE_APP_NAME.as_str(),
    );
    let dec_app_id = DecAppId::try_from(dec_app.desc().object_id()).unwrap();

    let mut admin_stacks: Vec<CyfsStack> = vec![];
    for ((admin, _), (device, private_key)) in admins.iter() {
        let cyfs_stack = create_stack(
            admin,
            private_key,
            device,
            admins
                .iter()
                .map(|m| (m.0 .0.clone(), m.1 .0.clone()))
                .collect(),
            members
                .iter()
                .map(|m| (m.0 .0.clone(), m.1 .0.clone()))
                .collect(),
            &group,
            &dec_app,
        )
        .await;
        admin_stacks.push(cyfs_stack);
    }

    async_std::task::sleep(Duration::from_millis(10000)).await;

    for i in 0..admin_stacks.len() {
        let stack = admin_stacks.get(i).unwrap();
        let ((admin, _), _) = admins.get(i).unwrap();
        DecService::run(
            &stack,
            admin.name().unwrap().to_string(),
            dec_app_id.clone(),
        )
        .await;

        let control = stack
            .group_mgr()
            .find_rpath_control(
                &group.desc().object_id(),
                dec_app_id.object_id(),
                &EXAMPLE_RPATH,
                IsCreateRPath::Yes,
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
        let owner = &admins.get(i % admins.len()).unwrap().0 .0;
        let proposal = create_proposal(
            i as u64,
            owner.desc().object_id(),
            group_id,
            dec_app_id.object_id().clone(),
        );

        let control = stack
            .group_mgr()
            .find_rpath_control(
                &group_id,
                dec_app_id.object_id(),
                &EXAMPLE_RPATH,
                IsCreateRPath::Yes,
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
                dec: dec_app_id.object_id().clone(),
                verified: None,
            },
            object: NONObjectInfo::new(proposal.desc().object_id(), buf, Some(proposal_any)),
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: Some(AccessString::full().value()),
        };
        noc.put_object(&req).await;
        proposals.push(proposal);
    }

    // futures::future::join_all(prepare_futures).await;

    log::info!("proposals prepared.");

    for i in 1..PROPOSAL_COUNT {
        let proposal = proposals.get(i - 1).unwrap().clone();
        let stack = admin_stacks.get(i % admin_stacks.len()).unwrap();

        let control = stack
            .group_mgr()
            .find_rpath_control(
                &group_id,
                dec_app_id.object_id(),
                &EXAMPLE_RPATH,
                IsCreateRPath::Yes,
            )
            .await
            .unwrap();

        async_std::task::spawn(async move {
            control.push_proposal(proposal).await.unwrap();
        });

        if i % 1 == 0 {
            async_std::task::sleep(Duration::from_millis(1000)).await;
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
