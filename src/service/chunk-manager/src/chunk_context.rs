use std::path::PathBuf;

use cyfs_base::*;

#[derive(Clone)]
pub struct ChunkContext{
    pub chunk_dir: PathBuf,
    pub device_id: DeviceId,
    pub device: Device,
    pub pri_key: PrivateKey
}

impl ChunkContext{
    pub fn get_device_id(&self,) -> DeviceId{
        self.device_id.clone()
    }

    pub fn get_device(&self) -> &Device{
        &self.device
    }

    pub fn get_public_key(&self,) -> &PublicKey{
        self.device.desc().public_key()
    }

    pub fn get_private_key(&self) -> &PrivateKey{
        &self.pri_key
    }
}