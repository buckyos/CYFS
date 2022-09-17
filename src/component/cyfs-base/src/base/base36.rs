use crate::{BuckyError, BuckyErrorCode, BuckyResult};

pub trait ToBase36 {
    fn to_base36(&self) -> String;
}

pub trait FromBase36 {
    fn from_base36(&self) -> BuckyResult<Vec<u8>>;
}

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnoqprstuvwxyz";

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

    #[test]
    fn test() {
        let id = ObjectId::default();
        let v = id.as_slice().to_base36();
        println!("{}", v);
        
        let s = "9cfBkPt8hb8rnaYJRZGsKc9ZS3ye3CixxCC13z9Ubswm";
        let id = ObjectId::from_str(s).unwrap();
        let v = id.to_base36();
        println!("{}", v);
        let id2 = ObjectId::from_base36(&v).unwrap();
        assert_eq!(id, id2);
        let id3 = ObjectId::from_str(&v).unwrap();
        assert_eq!(id, id3);

        let id2 = ObjectId::from_base36(&v.to_uppercase()).unwrap();
        assert_eq!(id, id2);
    }
}