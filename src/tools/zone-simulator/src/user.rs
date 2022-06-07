use cyfs_base::*;
use cyfs_cip::*;
use cyfs_stack_loader::*;

use std::path::Path;

#[derive(Debug)]
pub struct TestUserData {
    pub people_id: PeopleId,
    pub ood_id: DeviceId,
    pub standby_ood_id: Option<DeviceId>,
    pub device1_id: DeviceId,
    pub device2_id: DeviceId,
    pub user: TestUser,
}

#[derive(Debug)]
pub struct TestUserDataString {
    pub people_id: &'static str,
    pub ood_id: &'static str,
    pub standby_ood_id: Option<&'static str>,
    pub device1_id: &'static str,
    pub device2_id: &'static str,
}

impl TestUserData {
    pub fn check_equal(&self, data: &TestUserDataString) {
        assert_eq!(self.people_id.to_string(), data.people_id);
        assert_eq!(self.ood_id.to_string(), data.ood_id);
        assert_eq!(
            self.standby_ood_id.as_ref().map(|id| { id.to_string() }),
            data.standby_ood_id.as_ref().map(|v| { v.to_string() })
        );
        assert_eq!(self.device1_id.to_string(), data.device1_id);
        assert_eq!(self.device2_id.to_string(), data.device2_id);
    }
}

#[derive(Clone, Debug)]
pub struct TestUser {
    pub name: String,
    pub mnemonic: String,

    // people信息
    pub people: People,
    pub sk: PrivateKey,

    pub ood_work_mode: OODWorkMode,
    pub ood: DeviceInfo,
    pub standby_ood: Option<DeviceInfo>,

    pub device1: DeviceInfo,
    pub device2: DeviceInfo,
}

impl TestUser {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn dump(&self, dir: &Path) {
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(&dir.join("people.desc"), self.people.to_vec().unwrap()).unwrap();
        std::fs::write(&dir.join("people.sec"), self.sk.to_vec().unwrap()).unwrap();

        std::fs::write(&dir.join("ood.desc"), self.ood.device.to_vec().unwrap()).unwrap();
        std::fs::write(
            &dir.join("ood.sec"),
            self.ood.private_key.as_ref().unwrap().to_vec().unwrap(),
        )
        .unwrap();

        if let Some(ood) = &self.standby_ood {
            std::fs::write(&dir.join("standby_ood.desc"), ood.device.to_vec().unwrap()).unwrap();
            std::fs::write(
                &dir.join("standby_ood.sec"),
                ood.private_key.as_ref().unwrap().to_vec().unwrap(),
            )
            .unwrap();
        }

        std::fs::write(
            &dir.join("device1.desc"),
            self.device1.device.to_vec().unwrap(),
        )
        .unwrap();
        std::fs::write(
            &dir.join("device1.sec"),
            self.device1.private_key.as_ref().unwrap().to_vec().unwrap(),
        )
        .unwrap();

        std::fs::write(
            &dir.join("device2.desc"),
            self.device2.device.to_vec().unwrap(),
        )
        .unwrap();
        std::fs::write(
            &dir.join("device2.sec"),
            self.device2.private_key.as_ref().unwrap().to_vec().unwrap(),
        )
        .unwrap();
    }

    pub async fn new(name: &str, mnemonic: &str, index: u32, ood_work_mode: OODWorkMode) -> Self {
        let bip = CyfsSeedKeyBip::from_mnemonic(mnemonic, None).unwrap();

        // 创建people
        let path = CyfsChainBipPath::new_people(None, Some(index));
        let sk = bip.sub_key(&path).unwrap();

        let signer = RsaCPUObjectSigner::new(sk.public(), sk.clone());

        let builder = People::new(
            None,
            Vec::new(),
            sk.public(),
            None,
            Some(name.to_owned()),
            None,
        );
        let mut people = builder.no_create_time().build();
        let people_id = people.desc().calculate_id();

        info!("{}: people={}", name, people_id);

        // 签名
        cyfs_base::sign_and_set_named_object_desc(
            &signer,
            &mut people,
            &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
        )
        .await
        .unwrap();

        // 所有名下设备使用privateKey+people_id衍生而来，不再使用助记词
        let sk_hex = ::hex::encode(&sk.to_vec().unwrap());
        let bip = CyfsSeedKeyBip::from_private_key(&sk_hex, &people_id.to_string()).unwrap();

        // 创建ood
        let path = CyfsChainBipPath::new_device(0, None, Some(1));
        let ood_sk = bip.sub_key(&path).unwrap();
        let unique_id = format!("{}-ood", name);
        let unique_id = UniqueId::create_with_hash(unique_id.as_bytes());
        let builder = Device::new(
            Some(people_id.clone()),
            unique_id,
            vec![],
            vec![],
            vec![],
            ood_sk.public(),
            Area::default(),
            DeviceCategory::OOD,
        );
        let mut ood = builder.no_create_time().build();
        let ood_id = ood.desc().device_id();
        cyfs_base::sign_and_set_named_object_desc(
            &signer,
            &mut ood,
            &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
        )
        .await
        .unwrap();

        info!("{}: ood={}", name, ood_id);
        let ood_info = DeviceInfo {
            device: ood,
            private_key: Some(ood_sk),
        };

        // ood添加到people的ood_list
        people.ood_list_mut().push(ood_id.clone());
        people.set_ood_work_mode(OODWorkMode::Standalone);

        // 创建从OOD
        let mut standby_ood = None;
        if ood_work_mode == OODWorkMode::ActiveStandby {
            let path = CyfsChainBipPath::new_device(0, None, Some(2));
            let ood_sk = bip.sub_key(&path).unwrap();
            let unique_id = format!("{}-ood2", name);
            let unique_id = UniqueId::create_with_hash(unique_id.as_bytes());
            let builder = Device::new(
                Some(people_id.clone()),
                unique_id,
                vec![],
                vec![],
                vec![],
                ood_sk.public(),
                Area::default(),
                DeviceCategory::OOD,
            );
            let mut ood = builder.no_create_time().build();
            let s_ood_id = ood.desc().device_id();
            cyfs_base::sign_and_set_named_object_desc(
                &signer,
                &mut ood,
                &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
            )
            .await
            .unwrap();

            assert_ne!(ood_id, s_ood_id);
            info!("{}: standby ood={}", name, s_ood_id);
            standby_ood = Some(DeviceInfo {
                device: ood,
                private_key: Some(ood_sk),
            });

            // 从ood需要放到非首位位置
            people.ood_list_mut().push(s_ood_id.clone());
            people.set_ood_work_mode(OODWorkMode::ActiveStandby);
        }

        // 修改完people的body后，最后需要签名
        cyfs_base::sign_and_set_named_object_body(
            &signer,
            &mut people,
            &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
        )
        .await
        .unwrap();

        // 创建第一个device
        let path = CyfsChainBipPath::new_device(0, None, Some(2));
        let device1_sk = bip.sub_key(&path).unwrap();
        let unique_id = format!("{}-device1", name);
        let unique_id = UniqueId::create_with_hash(unique_id.as_bytes());
        let builder = Device::new(
            Some(people_id.clone()),
            unique_id,
            vec![],
            vec![],
            vec![],
            device1_sk.public(),
            Area::default(),
            DeviceCategory::AndroidMobile,
        );
        let mut device1 = builder.no_create_time().build();
        let device1_id = device1.desc().calculate_id();
        cyfs_base::sign_and_set_named_object_desc(
            &signer,
            &mut device1,
            &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
        )
        .await
        .unwrap();

        info!("{}: device1={}", name, device1_id);
        let device1_info = DeviceInfo {
            device: device1,
            private_key: Some(device1_sk),
        };

        // 创建第二个device
        let path = CyfsChainBipPath::new_device(0, None, Some(3));
        let device2_sk = bip.sub_key(&path).unwrap();
        let unique_id = format!("{}-device2", name);
        let unique_id = UniqueId::create_with_hash(unique_id.as_bytes());
        let builder = Device::new(
            Some(people_id.clone()),
            unique_id,
            vec![],
            vec![],
            vec![],
            device2_sk.public(),
            Area::default(),
            DeviceCategory::IOSMobile,
        );
        let mut device2 = builder.no_create_time().build();
        let device2_id = device2.desc().calculate_id();
        cyfs_base::sign_and_set_named_object_desc(
            &signer,
            &mut device2,
            &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
        )
        .await
        .unwrap();

        info!("{}: device2={}", name, device2_id);
        let device2_info = DeviceInfo {
            device: device2,
            private_key: Some(device2_sk),
        };

        Self {
            name: name.to_owned(),
            mnemonic: mnemonic.to_owned(),

            people,
            sk,

            ood_work_mode,
            ood: ood_info,
            standby_ood,

            device1: device1_info,
            device2: device2_info,
        }
    }

    pub fn user_data(&self) -> TestUserData {
        TestUserData {
            user: self.clone(),
            people_id: self.people.desc().people_id(),
            ood_id: self.ood.device.desc().device_id(),
            standby_ood_id: self
                .standby_ood
                .as_ref()
                .map(|info| info.device.desc().device_id()),
            device1_id: self.device1.device.desc().device_id(),
            device2_id: self.device2.device.desc().device_id(),
        }
    }
}
