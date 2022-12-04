
use cyfs_base::{Device, BuckyResult, ObjectId};

use super::k_bucket::{KadEntry, KBuckets, KBucketResult};

impl KadEntry for Device {
    fn newest_than(&self, _other: &Self) -> bool {
        false
    }
}

pub struct DeviceBucketes {
    array: KBuckets<ObjectId, Device>,
}

impl DeviceBucketes {
    pub fn new() -> Self {
        Self{
            array: KBuckets::new(7, ObjectId::default())
        }
    }

    pub fn set(&mut self, id: &ObjectId, device: &Device) -> BuckyResult<()> {
        match self.array.set(id, device) {
            KBucketResult::Added(_) => {},
            _ => {},
        }

        Ok(())
    }

    pub fn get_nearest_of(&self, id: &ObjectId) -> Option<&Device> {
        self.array
            .get_nearest_of(id)
            .get(0)
            .map(| (_, device) |*device)
    }
}
