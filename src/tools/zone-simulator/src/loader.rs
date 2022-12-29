use crate::profile::TEST_PROFILE;
use crate::user::*;
use crate::zone::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_stack_loader::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;


pub static USER1_DATA: OnceCell<TestUserData> = OnceCell::new();
pub static USER2_DATA: OnceCell<TestUserData> = OnceCell::new();

// 生成随机助记词
pub fn random_mnemonic() {
    use bip39::*;

    let mn = Mnemonic::generate_in(Language::English, 12).unwrap();
    println!("random mnemonic as follows:\n{}", mn.to_string());
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DeviceIndex {
    User1OOD,
    User1StandbyOOD,
    User1Device1,
    User1Device2,

    User2OOD,
    User2Device1,
    User2Device2,
}

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;
    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

lazy_static::lazy_static! {
    pub static ref DEC_ID: ObjectId = new_dec("zone-simulator");
}

pub struct TestLoader {}

impl TestLoader {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_dec_id() -> &'static ObjectId {
        &DEC_ID
    }

    pub fn get_id(index: DeviceIndex) -> String {
        let ret = match index {
            DeviceIndex::User1OOD => &USER1_DATA.get().unwrap().ood_id,
            DeviceIndex::User1StandbyOOD => {
                &USER1_DATA.get().unwrap().standby_ood_id.as_ref().unwrap()
            }
            DeviceIndex::User1Device1 => &USER1_DATA.get().unwrap().device1_id,
            DeviceIndex::User1Device2 => &USER1_DATA.get().unwrap().device2_id,

            DeviceIndex::User2OOD => &USER2_DATA.get().unwrap().ood_id,
            DeviceIndex::User2Device1 => &USER2_DATA.get().unwrap().device1_id,
            DeviceIndex::User2Device2 => &USER2_DATA.get().unwrap().device2_id,
        };

        ret.to_string()
    }

    pub fn get_stack(index: DeviceIndex) -> CyfsStack {
        let id = Self::get_id(index);

        CyfsServiceLoader::cyfs_stack(Some(&id))
    }

    pub fn get_user(index: DeviceIndex) -> &'static TestUser {
        match index {
            DeviceIndex::User1OOD
            | DeviceIndex::User1StandbyOOD
            | DeviceIndex::User1Device1
            | DeviceIndex::User1Device2 => &USER1_DATA.get().unwrap().user,

            DeviceIndex::User2OOD | DeviceIndex::User2Device1 | DeviceIndex::User2Device2 => {
                &USER2_DATA.get().unwrap().user
            }
        }
    }
    pub fn get_shared_stack(index: DeviceIndex) -> SharedCyfsStack {
        let id = Self::get_id(index);

        let stack = CyfsServiceLoader::shared_cyfs_stack(Some(&id));
        if stack.dec_id().is_none() {
            stack.bind_dec(DEC_ID.clone());
        }

        stack
    }

    pub async fn load_default(stack_config: &CyfsStackInsConfig) {
        let (user1, user2) = TestLoader::load_users(TEST_PROFILE.get_mnemonic(), true, false).await;

        TEST_PROFILE.save_desc();

        TestLoader::load_stack(stack_config, user1, user2).await;
    }

    pub async fn load_users(mnemonic: &str, as_default: bool, dump: bool) -> (TestUser, TestUser) {
        CyfsServiceLoader::prepare_env().await.unwrap();

        KNOWN_OBJECTS_MANAGER.clear();
        KNOWN_OBJECTS_MANAGER.set_mode(CyfsStackKnownObjectsInitMode::Sync);

        // 首先创建people/device信息组
        let (user1, user2) = Self::create_users(mnemonic).await;

        if as_default {
            USER1_DATA.set(user1.user_data()).unwrap();
            USER2_DATA.set(user2.user_data()).unwrap();
        }

        // 把相关对象添加到known列表
        Self::init_user_objects(&user1);
        Self::init_user_objects(&user2);

        if dump {
            let etc_dir = cyfs_util::get_service_config_dir("zone-simulator");
            info!(
                "dump user1 people & device .desc and .sec to {}",
                etc_dir.join("user1").display()
            );
            user1.dump(&etc_dir.join("user1"));
            info!(
                "dump user1 people & device .desc and .sec to {}",
                etc_dir.join("user2").display()
            );
            user2.dump(&etc_dir.join("user2"));
        }

        (user1, user2)
    }

    pub async fn load_stack(stack_config: &CyfsStackInsConfig, user1: TestUser, user2: TestUser) {
        use rand::Rng;

        let port: u16 = rand::thread_rng().gen_range(30000, 50000);

        // 初始化协议栈
        let config = stack_config.to_owned();
        let t1 = async_std::task::spawn(async move {
            let zone = TestZone::new(true, port, 21000, user1);
            zone.init(&config).await;
        });

        let config = stack_config.to_owned();
        let t2 = async_std::task::spawn(async move {
            let zone = TestZone::new(true, port + 10, 21010, user2);
            zone.init(&config).await;
        });

        ::futures::join!(t1, t2);

        info!(">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>\nload zone stacks complete!\n>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>");
    }

    fn init_user_objects(user: &TestUser) {
        let mut list = Vec::new();

        let obj = NONObjectInfo {
            object_id: user.people.desc().calculate_id(),
            object_raw: user.people.to_vec().unwrap(),
            object: Some(Arc::new(AnyNamedObject::Standard(StandardObject::People(
                user.people.clone(),
            )))),
        };
        list.push(obj);

        let obj = NONObjectInfo {
            object_id: user.ood.device.desc().calculate_id(),
            object_raw: user.ood.device.to_vec().unwrap(),
            object: Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                user.ood.device.clone(),
            )))),
        };
        list.push(obj);

        if let Some(info) = &user.standby_ood {
            let obj = NONObjectInfo {
                object_id: info.device.desc().calculate_id(),
                object_raw: info.device.to_vec().unwrap(),
                object: Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                    info.device.clone(),
                )))),
            };
            list.push(obj);
        }

        let obj = NONObjectInfo {
            object_id: user.device1.device.desc().calculate_id(),
            object_raw: user.device1.device.to_vec().unwrap(),
            object: Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                user.device1.device.clone(),
            )))),
        };
        list.push(obj);

        let obj = NONObjectInfo {
            object_id: user.device2.device.desc().calculate_id(),
            object_raw: user.device2.device.to_vec().unwrap(),
            object: Some(Arc::new(AnyNamedObject::Standard(StandardObject::Device(
                user.device2.device.clone(),
            )))),
        };
        list.push(obj);

        KNOWN_OBJECTS_MANAGER.append(list);
    }

    async fn create_users(mnemonic: &str) -> (TestUser, TestUser) {
        let mnemonic1 = mnemonic.to_owned();
        let t1 = async_std::task::spawn(async move {
            let user = TestUser::new("user001", &mnemonic1, 0, OODWorkMode::ActiveStandby).await;
            user
        });

        let mnemonic2 = mnemonic.to_owned();
        let t2 = async_std::task::spawn(async move {
            let user = TestUser::new("user002", &mnemonic2, 1, OODWorkMode::Standalone).await;
            user
        });

        ::futures::join!(t1, t2)
    }
}
