use crate::*;

pub struct ObjectDifficulty {}

impl ObjectDifficulty {
    pub fn difficulty(hash: &HashValue) -> u8 {
        let hash = hash.as_slice();
        let mut i = 0;
        let mut diff = 0;
        while i < hash.len() {
            let ret = Self::leading_zero(hash[i]);
            diff += ret;
            if ret < 8 {
                break;
            }

            i += 1;
        }

        diff
    }

    pub fn difficulty_with_factor(private_key_type: PrivateKeyFullType, difficulty: u8) -> u8 {
        if difficulty == 0 {
            return 0;
        }
        
        let factor = match private_key_type {
            PrivateKeyFullType::Rsa1024 => PrivateKeyDifficultyFactor::Rsa1024,
            PrivateKeyFullType::Rsa2048 => PrivateKeyDifficultyFactor::Rsa2048,
            PrivateKeyFullType::Rsa3072 => PrivateKeyDifficultyFactor::Rsa3072,
            PrivateKeyFullType::Secp256k1 => PrivateKeyDifficultyFactor::Secp256k1,
            PrivateKeyFullType::Unknown => PrivateKeyDifficultyFactor::Rsa1024,
        } as i8;

        let mut f_diff  = difficulty as isize + factor as isize;
        if f_diff > u8::MAX as isize {
            f_diff = u8::MAX as isize;
        } else if f_diff < 0 {
            f_diff = 0
        }

        let f_diff = f_diff as u8;
        info!("difficulty with factor: type={:?}, {} -> {}", private_key_type, difficulty, f_diff);
        f_diff
    }

    fn leading_zero(byte: u8) -> u8 {
        if byte == 0 {
            8
        } else {
            let mut count = 7;
            loop {
                let tmp = byte & (1 << count);
                // println!("count={},value={:b}", count, tmp);
                if tmp != 0 {
                    break;
                }

                if count == 0 {
                    break;
                }

                count -= 1;
            }

            7 - count as u8
        }
    }

    pub fn format_binary(hash: &[u8]) {
        for n in hash.iter() {
            print!("{:b} ", n);
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PrivateKeyFullType {
    Rsa1024,
    Rsa2048,
    Rsa3072,
    Secp256k1,
    Unknown,
}

impl PrivateKey {
    pub fn full_key_type(&self) -> PrivateKeyFullType {
        use rsa::PublicKeyParts;

        match self {
            Self::Rsa(v) => match v.size() {
                RSA_KEY_BYTES => PrivateKeyFullType::Rsa1024,
                RSA2048_KEY_BYTES => PrivateKeyFullType::Rsa2048,
                RSA3072_KEY_BYTES => PrivateKeyFullType::Rsa3072,
                _ => PrivateKeyFullType::Unknown,
            },
            Self::Secp256k1(_) => PrivateKeyFullType::Secp256k1,
        }
    }
}

impl PublicKey {
    pub fn full_key_type(&self) -> PrivateKeyFullType {
        use rsa::PublicKeyParts;

        match self {
            Self::Rsa(v) => match v.size() {
                RSA_KEY_BYTES => PrivateKeyFullType::Rsa1024,
                RSA2048_KEY_BYTES => PrivateKeyFullType::Rsa2048,
                RSA3072_KEY_BYTES => PrivateKeyFullType::Rsa3072,
                _ => PrivateKeyFullType::Unknown,
            },
            Self::Secp256k1(_) => PrivateKeyFullType::Secp256k1,
            Self::Invalid => PrivateKeyFullType::Unknown,
        }
    }

    pub fn difficulty_with_factor(&self, difficulty: u8) -> u8 {
        ObjectDifficulty::difficulty_with_factor(self.full_key_type(), difficulty)
    }
}

#[repr(i8)]
pub enum PrivateKeyDifficultyFactor {
    Rsa1024 = 0 /* 1000 as base */,
    Rsa2048 = -2 /* 5250 */,
    Rsa3072 = -4 /* 16000 */,
    Secp256k1 = 2 /* 250 */,
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn calc_diff() {
        assert_eq!(ObjectDifficulty::leading_zero(0), 8);
        assert_eq!(ObjectDifficulty::leading_zero(0b01111111), 1);
        assert_eq!(ObjectDifficulty::leading_zero(0b00111111), 2);
        assert_eq!(ObjectDifficulty::leading_zero(0b00011111), 3);
        assert_eq!(ObjectDifficulty::leading_zero(0b00001111), 4);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000111), 5);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000011), 6);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000001), 7);
        assert_eq!(ObjectDifficulty::leading_zero(0b00000000), 8);

        assert_eq!(ObjectDifficulty::leading_zero(0xFF), 0);
        assert_eq!(ObjectDifficulty::leading_zero(0x0F), 4);
    }

    fn calc_difficulty_with_factor(difficulty: u8) {
        let f_diff = ObjectDifficulty::difficulty_with_factor(PrivateKeyFullType::Rsa1024, difficulty);
        println!("difficulty with factor: type={:?}, {} -> {}", PrivateKeyFullType::Rsa1024, difficulty, f_diff);

        let f_diff = ObjectDifficulty::difficulty_with_factor(PrivateKeyFullType::Rsa2048, difficulty);
        println!("difficulty with factor: type={:?}, {} -> {}", PrivateKeyFullType::Rsa2048, difficulty, f_diff);

        let f_diff = ObjectDifficulty::difficulty_with_factor(PrivateKeyFullType::Rsa3072, difficulty);
        println!("difficulty with factor: type={:?}, {} -> {}", PrivateKeyFullType::Rsa3072, difficulty, f_diff);

        let f_diff = ObjectDifficulty::difficulty_with_factor(PrivateKeyFullType::Secp256k1, difficulty);
        println!("difficulty with factor: type={:?}, {} -> {}", PrivateKeyFullType::Secp256k1, difficulty, f_diff);
    }

    #[test]
    fn test_factor() {
        calc_difficulty_with_factor(0);
        calc_difficulty_with_factor(100);
        calc_difficulty_with_factor(1);
        calc_difficulty_with_factor(20);
    }
}
