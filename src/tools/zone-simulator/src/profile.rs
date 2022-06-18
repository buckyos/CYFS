use crate::loader::{USER1_DATA, USER2_DATA};

use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};

pub struct TestProfile {
    mnemonic: OnceCell<String>,
    etc_dir: PathBuf,
}

impl TestProfile {
    pub fn new() -> Self {
        let etc_dir = cyfs_util::get_service_config_dir("zone-simulator");
        std::fs::create_dir_all(&etc_dir).unwrap();

        Self {
            mnemonic: OnceCell::new(),
            etc_dir,
        }
    }

    pub fn set_mnemonic(&self, mnemonic: &str) {
        info!("set mnemonic: {}", mnemonic);

        self.mnemonic.set(mnemonic.to_owned()).unwrap();
    }

    pub fn load(&self) {
        let mn_file = self.etc_dir.join("mnemonic");
        let mn;
        if !mn_file.exists() {
            mn = Self::random_mnemonic(&mn_file);
        } else {
            mn = Self::load_mnemonic(&mn_file);
        }

        self.mnemonic.set(mn).unwrap();
    }

    pub fn get_mnemonic(&self) -> &str {
        self.mnemonic.get().unwrap()
    }

    pub fn save_desc(&self) {
        let file = self.etc_dir.join("desc_list");
        let user1 = USER1_DATA.get().unwrap();
        let user2 = USER2_DATA.get().unwrap();

        let s = format!(
            "zone1 as follows:\npeople:{}\nood:{}\nstandby_ood:{}\ndevice1:{}\ndevice2:{}\n\nzone2 as follows:\npeople:{}\nood:{}\ndevice1:{}\ndevice2:{}\n",
            user1.people_id,
            user1.ood_id,
            user1.standby_ood_id.as_ref().unwrap(),
            user1.device1_id,
            user1.device2_id,
            user2.people_id,
            user2.ood_id,
            user2.device1_id,
            user2.device2_id,
        );

        std::fs::write(file, &s).unwrap();
    }

    fn load_mnemonic(file: &Path) -> String {
        assert!(file.exists());

        let mn = std::fs::read_to_string(file).unwrap();
        info!(
            "load mnemonic from file: file={}, mnemonic={}",
            file.display(),
            mn
        );

        mn
    }

    fn random_mnemonic(file: &Path) -> String {
        use bip39::*;
        let mn = Mnemonic::generate_in(Language::English, 12).unwrap();
        let mn = mn.to_string();
        info!("random mnemonic: {}", mn);

        std::fs::write(file, &mn).unwrap();

        mn
    }
}

lazy_static::lazy_static! {
    pub static ref TEST_PROFILE: TestProfile = TestProfile::new();
}
