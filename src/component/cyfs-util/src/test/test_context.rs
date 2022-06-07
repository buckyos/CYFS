use crate::get_device_from_file;
use cyfs_base::*;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct TestContext {
    meta_endpoint: String,
    test_dir: PathBuf,
    owner_device_desc_path: PathBuf,
    owner_device_id: DeviceId,
    owowner_device: Device,
    owner_private_key: PrivateKey,
    devices: Vec<(Device, PrivateKey)>,
}

impl TestContext {
    pub fn new(
        meta_endpoint: String,
        test_dir: PathBuf,
        peer_desc_path: PathBuf,
        peer_desc: Device,
        peer_secret: PrivateKey,
    ) -> BuckyResult<Self> {
        let peer_id = peer_desc.desc().device_id();

        Ok(TestContext {
            meta_endpoint,
            test_dir,
            owner_device_desc_path: peer_desc_path,
            owner_device_id: peer_id,
            owowner_device: peer_desc,
            owner_private_key: peer_secret,
            devices: Vec::new(),
        })
    }

    pub fn owner_device(&self) -> &Device {
        &self.owowner_device
    }

    pub fn owner_device_desc_path(&self) -> PathBuf {
        self.owner_device_desc_path.clone()
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.owner_device_id.clone()
    }

    pub fn owner_public_key(&self) -> &PublicKey {
        self.owowner_device.desc().public_key()
    }

    pub fn owner_private_key(&self) -> &PrivateKey {
        &self.owner_private_key
    }

    pub fn test_dir(&self) -> PathBuf {
        self.test_dir.clone()
    }

    pub fn devices(&self) -> &Vec<(Device, PrivateKey)> {
        &self.devices
    }

    pub fn meta_endpoint(&self) -> String {
        self.meta_endpoint.clone()
    }

    pub fn load_devices(&mut self) -> BuckyResult<()> {
        let peer_desc_dir = self.test_dir.join("device_secs");
        info!(
            "load peers info from peer desc dir:{}",
            peer_desc_dir.to_string_lossy()
        );
        for entry in fs::read_dir(&peer_desc_dir)? {
            let entry = entry?;
            let peer_path = entry.path();

            let peer_desc_path = peer_path.join("device");

            info!(
                "load peer desc from peer desc file:{}",
                peer_desc_path.to_string_lossy()
            );
            let (device_desc, secret) = get_device_from_file(
                &peer_desc_path.with_extension("desc"),
                &peer_desc_path.with_extension("sec"),
            )?;

            self.devices.push((device_desc, secret));
        }
        Ok(())
    }
}
