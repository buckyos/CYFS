use crate::{BuckyError, BuckyErrorCode, BuckyResult};

pub use base58::{FromBase58, ToBase58};
pub trait ToBase36 {
    fn to_base36(&self) -> String;
}

pub trait FromBase36 {
    fn from_base36(&self) -> BuckyResult<Vec<u8>>;
}

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

impl ToBase36 for [u8] {
    fn to_base36(&self) -> String {
        base_x::encode(ALPHABET, self)
    }
}

impl FromBase36 for str {
    fn from_base36(&self) -> BuckyResult<Vec<u8>> {
        base_x::decode(ALPHABET, &self.to_ascii_lowercase()).map_err(|e| {
            let msg = format!("convert string to base36 error! {self}, {e}");
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }
}


#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::*;

    fn test_convert(s: &str) -> (ObjectId, String) {
        let id = ObjectId::from_str(s).unwrap();
        let v = id.to_base36();
        println!("{}", v);
        let id2 = ObjectId::from_base36(&v).unwrap();
        assert_eq!(id, id2);
        let id3 = ObjectId::from_str(&v).unwrap();
        assert_eq!(id, id3);

        let id2 = ObjectId::from_base36(&v.to_uppercase()).unwrap();
        assert_eq!(id, id2);

        (id, v)
    }

    #[test]
    fn test() {
        let id = ObjectId::default();
        let v = id.as_slice().to_base36();
        println!("{}", v);
        
        // let s = "9cfBkPt8hb8rnaYJRZGsKc9ZS3ye3CixxCC13z9Ubswm";
        let s = "9tGpLNna8UVtPYCfV1LbRN2Bqa5G9vRBKhDhZiWjd7wA";
        let (id1, id11) = test_convert(s);
        //assert_eq!(id11, "3afs9n7yl2qk43kusooijq9hmadmbil1162zeyi8o28aes3em5");

        let s = "9tGpLNna8P1hutR3y6i1gSGuosoLXLxa72HMrcEQnrgk";
        let (id2, id22) = test_convert(s);

        assert_ne!(id11, id22);
        assert_ne!(id1, id2);

        let s = "95RvaS5eWQsLpCGgkY773EKomgfa73EcmGP9VHWnwig3";
        test_convert(s);
    }
}