use crate::ObjectId;

pub trait CustomObjectId {
    fn from_slice_value(buf: &[u8]) -> Self;
    fn get_slice_value(&self) -> &[u8];
}

impl CustomObjectId for ObjectId {
    fn from_slice_value(buf: &[u8]) -> Self {
        assert!(buf.len() <= 31);
        let mut value = [0u8; 32];
        value[1..buf.len() + 1].copy_from_slice(buf);
        ObjectId::clone_from_slice(&value).unwrap()
    }

    fn get_slice_value(&self) -> &[u8] {
        &self.as_slice()[1..]
    }
}
