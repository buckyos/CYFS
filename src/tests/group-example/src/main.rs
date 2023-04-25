use std::{clone, collections::HashSet, sync::Arc, time::Duration};

use async_std::sync::Mutex;
use cyfs_base::{
    AccessString, AnyNamedObject, NamedObject, ObjectDesc, ObjectId, RawConvertTo, RawDecode,
    RawFrom, TypelessCoreObject,
};
use cyfs_core::{
    DecApp, DecAppId, DecAppObj, GroupProposal, GroupProposalObject, GroupRPath, Text, TextObj,
};
use cyfs_group_lib::GroupManager;
use cyfs_lib::{
    CyfsStackRequestorType, DeviceZoneCategory, DeviceZoneInfo, NONObjectInfo,
    NONOutputRequestCommon, NONPutObjectOutputRequest, NamedObjectCachePutObjectRequest,
    NamedObjectStorageCategory, RequestProtocol, RequestSourceInfo, SharedCyfsStack,
};
use cyfs_stack::CyfsStack;
use Common::{create_stack, EXAMPLE_RPATH};
use GroupDecService::DecService;

use crate::{
    Common::{init_admins, init_group, init_members, EXAMPLE_APP_NAME, EXAMPLE_VALUE_PATH},
    GroupDecService::MyRPathDelegate,
};

/**
 * Build the group for test
 * .\desc-tool create people --savepath=test-group/admins --idfile=test-group/admins/people-1.id
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/admins/people-1.sec -t=test-group/admins/people-1.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/admins/people-2.sec -t=test-group/admins/people-2.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/admins/people-3.sec -t=test-group/admins/people-3.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/admins/people-4.sec -t=test-group/admins/people-4.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-1.sec -t=test-group/members/people-1.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-2.sec -t=test-group/members/people-2.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-3.sec -t=test-group/members/people-3.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-4.sec -t=test-group/members/people-4.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-5.sec -t=test-group/members/people-5.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-6.sec -t=test-group/members/people-6.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-7.sec -t=test-group/members/people-7.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-8.sec -t=test-group/members/people-8.desc -dba
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=test-group/members/people-9.sec -t=test-group/members/people-9.desc -dba
 *
 * .\cyfs-meta-client.exe putdesc -c=test-group/admins/people-5 -d=test-group/admins/people-5.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/admins/people-6 -d=test-group/admins/people-6.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/admins/people-7 -d=test-group/admins/people-7.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/admins/people-8 -d=test-group/admins/people-8.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-10 -d=test-group/members/people-10.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-11 -d=test-group/members/people-11.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-12 -d=test-group/members/people-12.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-13 -d=test-group/members/people-13.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-14 -d=test-group/members/people-14.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-15 -d=test-group/members/people-15.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-16 -d=test-group/members/people-16.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-17 -d=test-group/members/people-17.desc 1 0
 * .\cyfs-meta-client.exe putdesc -c=test-group/members/people-18 -d=test-group/members/people-18.desc 1 0
 *
 * .\cyfs-meta-client.exe putdesc -c=test-group/admins/people-1.desc -d=67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc 0 0
 *
 * .\desc-tool show -a 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc
 *
 * .\desc-tool modify 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc --add_admin=5r4MYfFBsQqy4r2LTccK1yyipRTtAjqvhX3GLU2qX3Lo --add_member=5r4MYfFapPzrXhfxWJNZJf4pk5Ncfrx5ax2yumWKZrZj
 * .\desc-tool modify 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc --add_ood=5aSixgNLsF6r3qjDKP3XkBwnDRSr5G5hrGRM2v2LnLoA
 * .\desc-tool modify 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc --prev_shell=9cfBkPt2RPa3MofMmsAXpq8xHYn8A2xvVPuQiBT4XTp9 -v=3
 * .\desc-tool sign 67fz8e9wt6Yqb9ex61tr2C1PwCqavBLFe153wfiVAhqQ.desc -s=
 *
*/

mod Common {
    use std::{
        fmt::format, io::ErrorKind, net::SocketAddrV4, sync::Arc, thread::sleep, time::Duration,
    };

    use async_std::fs;
    use cyfs_base::{
        AnyNamedObject, Area, Device, DeviceCategory, DeviceId, Endpoint, EndpointArea, Group,
        GroupMember, IpAddr, NamedObject, ObjectDesc, ObjectLink, People, PrivateKey, Protocol,
        RawConvertTo, RawDecode, RawEncode, RawFrom, RsaCPUObjectSigner, Signer, SocketAddr,
        StandardObject, TypelessCoreObject, UniqueId, NON_STACK_BDT_VPORT,
        NON_STACK_SYNC_BDT_VPORT, SIGNATURE_SOURCE_REFINDEX_OWNER, SIGNATURE_SOURCE_REFINDEX_SELF,
    };
    use cyfs_bdt_ext::{BdtStackParams, SNMode};
    use cyfs_chunk_lib::ChunkMeta;
    use cyfs_core::{DecApp, DecAppId, ToGroupShell};
    use cyfs_lib::{BrowserSanboxMode, NONObjectInfo, SharedCyfsStack};
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
        pub static ref EXAMPLE_RPATH: String = "rpath-example-7".to_string();
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
        admins: Vec<(&People, &PrivateKey)>,
        members: Vec<(&People, &PrivateKey)>,
        oods: Vec<&Device>,
    ) -> Group {
        log::info!("create group");

        let mut group = Group::new_org(Some(founder.desc().object_id()), Area::default()).build();
        group.check_org_body_content_mut().set_admins(
            admins
                .iter()
                .map(|m| GroupMember::from_member_id(m.0.desc().object_id()))
                .collect(),
        );
        group.set_members(
            members
                .iter()
                .map(|m| GroupMember::from_member_id(m.0.desc().object_id()))
                .collect(),
        );
        group.set_ood_list(
            oods.iter()
                .map(|d| DeviceId::try_from(d.desc().object_id()).unwrap())
                .collect(),
        );

        log::info!("create group: {:?}", group.desc().object_id());

        let desc_hash = group.desc().raw_hash_value().unwrap();
        let body_hash = group.body().as_ref().unwrap().raw_hash_value().unwrap();
        let signers = [admins, members].concat();
        signers
            .into_iter()
            .map(|(owner, private_key)| {
                let signer = RsaCPUObjectSigner::new(private_key.public(), private_key.clone());

                async_std::task::block_on(async move {
                    let desc_signature = signer
                        .sign(
                            desc_hash.as_slice(),
                            &cyfs_base::SignatureSource::Object(ObjectLink {
                                obj_id: owner.desc().object_id(),
                                obj_owner: None,
                            }),
                        )
                        .await
                        .unwrap();

                    let body_signature = signer
                        .sign(
                            body_hash.as_slice(),
                            &cyfs_base::SignatureSource::Object(ObjectLink {
                                obj_id: owner.desc().object_id(),
                                obj_owner: None,
                            }),
                        )
                        .await
                        .unwrap();
                    (desc_signature, body_signature)
                })
            })
            .for_each(|(desc_signature, body_signature)| {
                group.signs_mut().push_desc_sign(desc_signature);
                group.signs_mut().push_body_sign(body_signature);
            });

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
        init_member_from_dir("./test-group/admins", "admin", 5, min_port).await
    }

    pub async fn init_members() -> Vec<((People, PrivateKey), (Device, PrivateKey))> {
        let min_port = 31217_u16;
        init_member_from_dir("./test-group/members", "member", 10, min_port).await
    }

    pub async fn init_group(
        admins: Vec<(&People, &PrivateKey)>,
        members: Vec<(&People, &PrivateKey)>,
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

        let group = create_group(admins.get(0).unwrap().0, admins, members, oods);
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
        rpc_port: u16,
        ws_port: u16,
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
            sn_mode: SNMode::Normal,
        };

        let stack_param = CyfsStackParams {
            config: CyfsStackConfigParams {
                isolate: Some(device.desc().object_id().to_string()),
                sync_service: false,
                shared_stack: true,
                perf_service: false,
            },
            noc: CyfsStackNOCParams {},
            interface: CyfsStackInterfaceParams {
                bdt_listeners: vec![NON_STACK_BDT_VPORT, NON_STACK_SYNC_BDT_VPORT],
                tcp_listeners: vec![SocketAddr::V4(SocketAddrV4::new(
                    std::net::Ipv4Addr::new(127, 0, 0, 1),
                    rpc_port,
                ))],
                ws_listener: Some(SocketAddr::V4(SocketAddrV4::new(
                    std::net::Ipv4Addr::new(127, 0, 0, 1),
                    ws_port,
                ))),
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

        let group_shell = group.to_shell();
        let group_shell_vec = group_shell.to_vec().unwrap();
        let typeless = TypelessCoreObject::clone_from_slice(group_shell_vec.as_slice()).unwrap();
        known_objects.list.push(NONObjectInfo::new(
            group_shell.shell_id(),
            group_shell_vec,
            Some(Arc::new(AnyNamedObject::Core(typeless))),
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
        rpc_port: u16,
        ws_port: u16,
    ) -> (Box<CyfsStack>, Box<SharedCyfsStack>) {
        let params = init_stack_params(
            people,
            private_key,
            device,
            admins,
            members,
            group,
            dec_app,
            rpc_port,
            ws_port,
        );

        log::info!("cyfs-stack.open");

        let stack = Box::new(
            CyfsStack::open(params.0, params.1, params.2)
                .await
                .map_err(|e| {
                    log::error!("stack start failed: {}", e);
                    e
                })
                .unwrap(),
        );

        async_std::task::sleep(Duration::from_millis(1000)).await;

        let shared_stack = Box::new(
            SharedCyfsStack::open_with_port(Some(dec_app.desc().object_id()), rpc_port, ws_port)
                .await
                .unwrap(),
        );

        shared_stack.wait_online(None).await.unwrap();

        (stack, shared_stack)
    }
}

mod Client {
    // use cyfs_base::ObjectId;
    // use cyfs_core::GroupProposal;
    // use cyfs_group_lib::RPathClient;

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
        CoreObjectType, DecAppId, GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal,
        GroupProposalObject, Text, TextObj,
    };
    use cyfs_group_lib::{
        DelegateFactory, ExecuteResult, GroupManager, GroupObjectMapProcessor, RPathDelegate,
        RPathService,
    };
    use cyfs_lib::{
        CreateObjectMapOption, GlobalStatePathAccessItem, NONAPILevel, NONGetObjectInputRequest,
        NONGetObjectOutputRequest, NONInputRequestCommon, NONObjectInfo, NONOutputRequestCommon,
        NONPostObjectInputRequest, NONPostObjectInputResponse, RequestGlobalStatePath,
        RequestSourceInfo, RootStateOpEnvAccess, RouterHandlerAction, RouterHandlerChain,
        RouterHandlerManagerProcessor, RouterHandlerPostObjectRequest,
        RouterHandlerPostObjectResult, SharedCyfsStack,
    };
    use cyfs_util::EventListenerAsyncRoutine;

    use crate::Common::{EXAMPLE_VALUE_PATH, STATE_PATH_SEPARATOR};

    pub struct DecService {}

    impl DecService {
        pub async fn run(
            cyfs_stack: &SharedCyfsStack,
            local_name: String,
            dec_app_id: DecAppId,
            group_id: ObjectId,
            rpath: &str,
        ) -> GroupManager {
            let group_mgr = GroupManager::open(
                cyfs_stack.clone(),
                Box::new(GroupRPathDelegateFactory {
                    local_name: local_name.clone(),
                    stack: cyfs_stack.clone(),
                    dec_id: dec_app_id.object_id().clone(),
                }),
                &cyfs_lib::CyfsStackRequestorType::Http,
            )
            .await
            .unwrap();

            let filter = format!("obj_type == {}", CoreObjectType::GroupProposal as u16,);
            let routine = ProposalListener {
                service: group_mgr
                    .start_rpath_service(
                        group_id,
                        rpath.to_string(),
                        Box::new(MyRPathDelegate::new(
                            local_name.to_string(),
                            cyfs_stack.clone(),
                            dec_app_id.object_id().clone(),
                        )),
                    )
                    .await
                    .unwrap(),
                local_name,
            };

            let req_path = RequestGlobalStatePath::new(
                Some(dec_app_id.object_id().clone()),
                Option::<String>::None,
            )
            .format_string();

            cyfs_stack
                .root_state_meta_stub(None, None)
                .add_access(GlobalStatePathAccessItem::new(
                    "group/proposal",
                    AccessString::full().value(),
                ))
                .await
                .unwrap();

            cyfs_stack
                .router_handlers()
                .post_object()
                .add_handler(
                    RouterHandlerChain::Handler,
                    format!("group-proposal-listener-{}", dec_app_id).as_str(),
                    0,
                    Some(filter),
                    Some(req_path),
                    RouterHandlerAction::Pass,
                    Some(Box::new(routine)),
                )
                .await
                .unwrap();

            group_mgr
        }
    }

    pub struct ProposalListener {
        service: RPathService,
        local_name: String,
    }

    #[async_trait::async_trait]
    impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
        for ProposalListener
    {
        async fn call(
            &self,
            param: &RouterHandlerPostObjectRequest,
        ) -> BuckyResult<RouterHandlerPostObjectResult> {
            log::info!(
                "recv proposal {} from {:?}, local: {}",
                param.request.object.object_id,
                param.request.common.source.zone,
                self.local_name
            );

            let (proposal, remain) =
                GroupProposal::raw_decode(param.request.object.object_raw.as_slice())?;
            assert_eq!(remain.len(), 0);

            let result = self.service.push_proposal(&proposal).await;

            Ok(RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(result.map(|result| NONPostObjectInputResponse { object: result })),
            })
        }
    }

    pub struct GroupRPathDelegateFactory {
        local_name: String,
        stack: SharedCyfsStack,
        dec_id: ObjectId,
    }

    impl GroupRPathDelegateFactory {
        pub fn is_accept(
            &self,
            group_id: &ObjectId,
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
            group_id: &ObjectId,
            rpath: &str,
            with_block: Option<&GroupConsensusBlock>,
            is_new: bool,
        ) -> BuckyResult<Box<dyn RPathDelegate>> {
            if self.is_accept(group_id, rpath, with_block) {
                // 如果接受，就提供该rpath的处理响应对象
                Ok(Box::new(MyRPathDelegate::new(
                    self.local_name.clone(),
                    self.stack.clone(),
                    self.dec_id.clone(),
                )))
            } else {
                Err(BuckyError::new(BuckyErrorCode::Reject, ""))
            }
        }
    }

    pub struct MyRPathDelegate {
        local_name: String,
        stack: SharedCyfsStack,
        dec_id: ObjectId,
        finished_proposals: Arc<Mutex<HashSet<ObjectId>>>,
    }

    impl MyRPathDelegate {
        pub fn new(local_name: String, stack: SharedCyfsStack, dec_id: ObjectId) -> Self {
            MyRPathDelegate {
                local_name,
                finished_proposals: Arc::new(Mutex::new(HashSet::new())),
                stack,
                dec_id,
            }
        }
    }

    impl MyRPathDelegate {
        pub async fn execute(
            &self,
            proposal: &GroupProposal,
            prev_state_id: &Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) -> BuckyResult<ExecuteResult> {
            let state_op_env = object_map_processor
                .create_sub_tree_op_env(Some(RootStateOpEnvAccess {
                    path: "".to_string(),
                    access: AccessPermissions::Full,
                }))
                .await
                .expect(format!("create_sub_tree_op_env failed").as_str());
            let prev_value = match prev_state_id {
                Some(prev_state_id) => {
                    state_op_env.load(prev_state_id.to_owned()).await?;
                    state_op_env
                        .get_by_path(EXAMPLE_VALUE_PATH.as_str())
                        .await
                        .expect(
                            format!("get_by_path {} failed", EXAMPLE_VALUE_PATH.as_str()).as_str(),
                        )
                }
                None => {
                    state_op_env
                        .create_new_with_option(
                            ObjectMapSimpleContentType::Map,
                            &CreateObjectMapOption::new_with_owner(
                                proposal.rpath().group_id().clone(),
                            ),
                        )
                        .await
                        .expect(
                            format!("create_new {} failed", EXAMPLE_VALUE_PATH.as_str()).as_str(),
                        );
                    None
                }
            };

            let (result_value, result_value_u64) = {
                /**
                 * prev_state_id是一个MAP的操作对象，形式待定，可能就是一个SingleOpEnv，但最好支持多级路径操作
                 */
                let prev_value = prev_value.map_or(0, |prev_value| {
                    let buf = prev_value.data();
                    let mut prev_value = [0u8; 8];
                    prev_value.copy_from_slice(&buf[..8]);
                    u64::from_be_bytes(prev_value)
                });

                let delta_buf = proposal.params().as_ref().unwrap().as_slice();
                let mut delta = [0u8; 8];
                delta.copy_from_slice(delta_buf);
                let delta = u64::from_be_bytes(delta);

                let value = prev_value + delta;
                let result_value = ObjectIdDataBuilder::new()
                    .data(&value.to_be_bytes())
                    .build()
                    .unwrap();
                (result_value, value)
            };

            let result_state_id = {
                let mut obj_map_id = result_value;
                for key in EXAMPLE_VALUE_PATH.split('/').rev() {
                    let key: &str = key.into();
                    if key.is_empty() {
                        continue;
                    }

                    let state_op_env = object_map_processor
                        .create_single_op_env(Some(RootStateOpEnvAccess {
                            path: "".to_string(),
                            access: AccessPermissions::Full,
                        }))
                        .await
                        .expect(format!("create_sub_tree_op_env failed").as_str());

                    state_op_env
                        .create_new_with_option(
                            ObjectMapSimpleContentType::Map,
                            &CreateObjectMapOption::new_with_owner(
                                proposal.rpath().group_id().clone(),
                            ),
                        )
                        .await
                        .expect(format!("create_new {} failed", key).as_str());

                    state_op_env
                        .insert_with_key(key, &obj_map_id)
                        .await
                        .expect(format!("insert with key {} failed", key).as_str());

                    obj_map_id = state_op_env
                        .commit()
                        .await
                        .expect(format!("commit key {} failed", key).as_str());
                }
                obj_map_id
            };

            let receipt = {
                /**
                 * 返回给Client的对象，相当于这个请求的结果或者叫回执？
                 */
                let text = Text::build("value", "header", format!("{}", result_value_u64))
                    .no_create_time()
                    .build();
                Some(NONObjectInfo::new(
                    text.desc().object_id(),
                    text.to_vec().unwrap(),
                    None,
                ))
            };

            let context = {
                /**
                 * 执行请求的上下文，运算过程中可能有验证节点无法得到的上下文信息（比如时间戳，随机数）
                 */
                Some(Vec::from(result_value_u64.to_le_bytes()))
            };

            /**
             * (result_state_id, return_object) = prev_value + proposal + context
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
            prev_state_id: &Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<()> {
            /**
             * let is_same = (execute_result.result_state_id, execute_result.return_object)
             *  == prev_state_id + proposal + execute_result.context
             */

            log::info!(
                "verify({}) enter, expect: prev-state: {:?}, {:?}/{:?}/{:?}",
                self.stack.local_device_id(),
                prev_state_id,
                execute_result.result_state_id,
                execute_result.context.as_ref().map(|v| v.to_hex()),
                execute_result.receipt.as_ref().map(|r| r.object_id),
            );

            let result = self
                .execute(proposal, prev_state_id, object_map_processor)
                .await?;

            log::info!(
                "verify({}) expect: prev-state: {:?}, {:?}/{:?}/{:?}, got: {:?}/{:?}/{:?}",
                self.stack.local_device_id(),
                prev_state_id,
                execute_result.result_state_id,
                execute_result.context.as_ref().map(|v| v.to_hex()),
                execute_result.receipt.as_ref().map(|r| r.object_id),
                result.result_state_id,
                result.context.as_ref().map(|v| v.to_hex()),
                result.receipt.as_ref().map(|r| r.object_id)
            );

            let is_ok = execute_result.result_state_id == result.result_state_id
                && execute_result.context == result.context
                && execute_result.receipt.as_ref().map(|r| r.object_id)
                    == result.receipt.as_ref().map(|r| r.object_id);

            if is_ok {
                Ok(())
            } else {
                Err(BuckyError::new(BuckyErrorCode::Reject, "result unmatch"))
            }
        }
    }

    #[async_trait::async_trait]
    impl RPathDelegate for MyRPathDelegate {
        async fn on_execute(
            &self,
            proposal: &GroupProposal,
            prev_state_id: &Option<cyfs_base::ObjectId>,
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) -> BuckyResult<ExecuteResult> {
            log::info!(
                "execute({}) enter, proposal: {}, prev-state: {:?}",
                self.stack.local_device_id(),
                proposal.desc().object_id(),
                prev_state_id,
            );

            let result = self
                .execute(proposal, prev_state_id, object_map_processor)
                .await?;

            log::info!(
                "execute({}), proposal: {}, prev-state: {:?}, result: {:?}",
                self.stack.local_device_id(),
                proposal.desc().object_id(),
                prev_state_id,
                result.result_state_id,
            );

            Ok(result)
        }

        async fn on_verify(
            &self,
            proposal: &GroupProposal,
            prev_state_id: &Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) -> BuckyResult<()> {
            self.verify(
                proposal,
                prev_state_id,
                object_map_processor,
                execute_result,
            )
            .await
        }

        async fn on_commited(
            &self,
            prev_state_id: &Option<cyfs_base::ObjectId>,
            block: &GroupConsensusBlock,
            object_map_processor: &dyn GroupObjectMapProcessor,
        ) {
            // 提交到共识链上了，可能有些善后事宜

            let prev_value = match prev_state_id {
                Some(prev_state_id) => {
                    let state_op_env = object_map_processor
                        .create_sub_tree_op_env(Some(RootStateOpEnvAccess {
                            path: "".to_string(),
                            access: AccessPermissions::Full,
                        }))
                        .await
                        .expect(format!("create_sub_tree_op_env failed").as_str());
                    state_op_env
                        .load(*prev_state_id)
                        .await
                        .expect(format!("load {} failed", prev_state_id).as_str());
                    state_op_env
                        .get_by_path(EXAMPLE_VALUE_PATH.as_str())
                        .await
                        .expect(
                            format!("get_by_path {:?} failed", EXAMPLE_VALUE_PATH.as_str())
                                .as_str(),
                        )
                }
                None => None,
            }
            .map_or(0, |prev_state_id| {
                let buf = prev_state_id.data();
                let mut prev_value = [0u8; 8];
                prev_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(prev_value)
            });

            let result_value = match block.result_state_id() {
                Some(result_state_id) => {
                    let state_op_env = object_map_processor
                        .create_sub_tree_op_env(Some(RootStateOpEnvAccess {
                            path: "".to_string(),
                            access: AccessPermissions::Full,
                        }))
                        .await
                        .expect(format!("create_sub_tree_op_env failed").as_str());
                    state_op_env
                        .load(result_state_id.clone())
                        .await
                        .expect(format!("load {} failed", result_state_id).as_str());
                    state_op_env
                        .get_by_path(EXAMPLE_VALUE_PATH.as_str())
                        .await
                        .expect(
                            format!("get_by_path {:?} failed", EXAMPLE_VALUE_PATH.as_str())
                                .as_str(),
                        )
                }
                None => None,
            }
            .map_or(0, |result_id| {
                let buf = result_id.data();
                let mut result_value = [0u8; 8];
                result_value.copy_from_slice(&buf[..8]);
                u64::from_be_bytes(result_value)
            });

            let proposal_infos =
                futures::future::join_all(block.proposals().iter().map(|proposal_info| async {
                    let proposal = self
                        .stack
                        .non_service()
                        .get_object(NONGetObjectOutputRequest {
                            common: NONOutputRequestCommon {
                                req_path: None,
                                source: None,
                                dec_id: Some(self.dec_id),
                                level: NONAPILevel::Router,
                                target: Some(block.owner().clone()),
                                flags: 0,
                            },
                            object_id: proposal_info.proposal,
                            inner_path: None,
                        })
                        .await
                        .unwrap();
                    let proposal = proposal.object;
                    let (proposal, _remain) =
                        GroupProposal::raw_decode(proposal.object_raw.as_slice()).unwrap();

                    let delta_buf = proposal.params().as_ref().unwrap().as_slice();
                    let mut delta = [0u8; 8];
                    delta.copy_from_slice(delta_buf);
                    let delta = u64::from_be_bytes(delta);
                    (proposal_info.proposal, delta)
                }))
                .await;

            log::info!(
                "proposal commited: height: {}/{}, delta: {:?}, result: {} -> {}, proposal: {:?}, block: {}, local: {}",
                block.height(), block.round(),
                proposal_infos.iter().map(|(_, delta)| *delta).collect::<Vec<_>>(),
                prev_value,
                result_value,
                proposal_infos.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                block.block_id(),
                self.local_name
            );

            let mut finished_proposals = self.finished_proposals.lock().await;
            block.proposals().iter().for_each(|proposal_info| {
                let is_new_finished = finished_proposals.insert(proposal_info.proposal);
                assert!(is_new_finished);
            });
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

    // async_std::task::sleep(Duration::from_millis(10000)).await;

    cyfs_debug::CyfsLoggerBuilder::new_app(EXAMPLE_APP_NAME.as_str())
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new(EXAMPLE_APP_NAME.as_str(), EXAMPLE_APP_NAME.as_str())
        .exit_on_panic(true)
        .dingtalk_bug_report("any value to disable")
        .build()
        .start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    log::info!("will open stacks");

    let admins = init_admins().await;
    let members = init_members().await;
    let group = init_group(
        admins.iter().map(|m| (&m.0 .0, &m.0 .1)).collect(),
        members.iter().map(|m| (&m.0 .0, &m.0 .1)).collect(),
        admins.iter().map(|m| &m.1 .0).collect(),
    )
    .await;
    let group_id = group.desc().object_id();
    let dec_app = DecApp::create(
        admins.get(0).unwrap().0 .0.desc().object_id(),
        EXAMPLE_APP_NAME.as_str(),
    );
    let dec_app_id = DecAppId::try_from(dec_app.desc().object_id()).unwrap();

    let mut admin_stacks: Vec<(Box<CyfsStack>, Box<SharedCyfsStack>)> = vec![];
    let mut admin_group_mgrs: Vec<GroupManager> = vec![];
    let mut member_stacks: Vec<(Box<CyfsStack>, Box<SharedCyfsStack>)> = vec![];
    let mut member_group_mgrs: Vec<GroupManager> = vec![];
    let mut rpc_port = 32217_u16;
    let mut ws_port = 33217_u16;
    for ((admin, _), (device, private_key)) in admins.iter() {
        let (cyfs_stack, shared_stack) = create_stack(
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
            rpc_port,
            ws_port,
        )
        .await;
        admin_stacks.push((cyfs_stack, shared_stack));
        rpc_port += 1;
        ws_port += 1;
    }

    log::info!("stacks for admins has opened.");

    for ((member, _), (device, private_key)) in members.iter() {
        let (cyfs_stack, shared_stack) = create_stack(
            member,
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
            rpc_port,
            ws_port,
        )
        .await;
        member_stacks.push((cyfs_stack, shared_stack));
        rpc_port += 1;
        ws_port += 1;
    }

    log::info!("stacks for members has opened.");

    async_std::task::sleep(Duration::from_millis(10000)).await;

    for i in 0..admin_stacks.len() {
        let (_, shared_stack) = admin_stacks.get(i).unwrap();
        let ((admin, _), (admin_device, _)) = admins.get(i).unwrap();
        let local_name = admin.name().unwrap();

        log::info!(
            "will start service, admin: {}, name: {}, device: {}",
            admin.desc().object_id(),
            local_name,
            admin_device.desc().object_id()
        );

        let group_mgr = DecService::run(
            &shared_stack,
            local_name.to_string(),
            dec_app_id.clone(),
            group_id,
            EXAMPLE_RPATH.as_str(),
        )
        .await;

        admin_group_mgrs.push(group_mgr);
    }

    log::info!("test dec-service for admins has opened.");

    for i in 0..member_stacks.len() {
        let (_, shared_stack) = member_stacks.get(i).unwrap();
        let ((member, _), _) = members.get(i).unwrap();
        let group_mgr = GroupManager::open_as_client(
            shared_stack.as_ref().clone(),
            &CyfsStackRequestorType::WebSocket,
        )
        .await
        .unwrap();

        member_group_mgrs.push(group_mgr);
    }

    log::info!("test dec-client for members has opened.");

    // async_std::task::sleep(Duration::from_millis(10000)).await;

    let mut proposals: Vec<GroupProposal> = vec![];

    log::info!("proposals will be prepared.");

    let PROPOSAL_COUNT = 20000usize;
    for i in 1..PROPOSAL_COUNT {
        let (_, stack) = member_stacks.get(i % member_stacks.len()).unwrap();
        let group_mgr = member_group_mgrs.get(i % member_group_mgrs.len()).unwrap();
        let owner = &members.get(i % members.len()).unwrap().0 .0;
        let proposal = create_proposal(
            i as u64,
            owner.desc().object_id(),
            group_id,
            dec_app_id.object_id().clone(),
        );

        let noc = stack.non_service().clone();

        let buf = proposal.to_vec().unwrap();
        let proposal_any = Arc::new(AnyNamedObject::Core(
            TypelessCoreObject::clone_from_slice(buf.as_slice()).unwrap(),
        ));

        let req = NONPutObjectOutputRequest {
            common: NONOutputRequestCommon {
                req_path: None,
                source: None,
                dec_id: None,
                level: cyfs_lib::NONAPILevel::NOC,
                target: None,
                flags: 0,
            },
            object: NONObjectInfo::new(proposal.desc().object_id(), buf, Some(proposal_any)),
            access: Some(AccessString::full()),
        };
        noc.put_object(req).await;
        proposals.push(proposal);

        let proposal = proposals.get(i - 1).unwrap().clone();
        let stack = member_stacks.get(i % member_stacks.len()).unwrap();
        let group_mgr = member_group_mgrs.get(i % member_group_mgrs.len()).unwrap();
        let ((member, _), _) = members.get(i % members.len()).unwrap();
        let local_name = member.name().map(|n| n.to_string());

        let client = group_mgr
            .rpath_client(group_id, dec_app_id.clone(), &EXAMPLE_RPATH)
            .await;

        async_std::task::spawn(async move {
            log::info!(
                "client {:?} will post proposal {}",
                local_name,
                proposal.desc().object_id(),
            );

            let result = client.post_proposal(&proposal).await;
            let result_text = result.as_ref().map(|obj| {
                obj.as_ref().map(|obj| {
                    Text::raw_decode(obj.object_raw.as_slice())
                        .map(|(txt, _)| txt.value().to_string())
                })
            });
            log::info!(
                "client {:?} post proposal {}, result: {:?}, result-text: {:?}",
                local_name,
                proposal.desc().object_id(),
                result.as_ref().map(|o| o.as_ref().map(|o| o.object_id)),
                result_text
            );
        });

        if i % 1 == 0 {
            async_std::task::sleep(Duration::from_millis(4000)).await;
            log::info!("will push new proposals, i: {}", i);
        }
    }

    // futures::future::join_all(prepare_futures).await;

    log::info!("proposals prepared.");

    // for i in 1..PROPOSAL_COUNT {}

    async_std::task::sleep(Duration::from_millis(20000)).await;

    // let client = admin_group_mgrs
    //     .get(0)
    //     .unwrap()
    //     .rpath_client(
    //         &group.desc().object_id(),
    //         dec_app_id.object_id(),
    //         &EXAMPLE_RPATH,
    //     )
    //     .await;

    // let value_obj = client
    //     .get_by_path(EXAMPLE_VALUE_PATH.as_str())
    //     .await
    //     .unwrap();
    // let buf = value_obj.as_ref().unwrap().object_id.data();
    // let mut value = [0u8; 8];
    // value.copy_from_slice(&buf[..8]);

    // log::info!("value from client is: {}", u64::from_be_bytes(value));
}

fn main() {
    log::info!("main");

    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    log::info!("will main-run");

    let fut = Box::pin(main_run());
    async_std::task::block_on(async move { fut.await })
}
