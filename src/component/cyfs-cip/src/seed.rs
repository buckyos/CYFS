use bip39::Mnemonic;
use std::fmt;
use hmac::Hmac;


const PBKDF2_ROUNDS: u32 = 2048;
const PBKDF2_BYTES: usize = 64;

#[derive(Clone)]
pub struct Seed {
    bytes: Vec<u8>,
}

impl Seed {
    fn pbkdf2(input: &[u8], salt: &str) -> Vec<u8> {
        let mut seed = vec![0u8; PBKDF2_BYTES];
    
        pbkdf2::pbkdf2::<Hmac<sha2::Sha512>>(input, salt.as_bytes(), PBKDF2_ROUNDS, &mut seed);
    
        seed
    }

    pub fn new(mnemonic: &Mnemonic, password: &str) -> Self {
        let salt = format!("cyfs-mnemonic-{}", password);

        let en = mnemonic.to_entropy();
        let bytes = Self::pbkdf2(&en, &salt);

        Self {
            bytes,
        }
    }

    pub fn new_from_private_key(private_key: &str, password: &str) -> Self {
        let salt = format!("cyfs-mnemonic-{}", password);
        let private_key = hex::decode(private_key).expect("invalid hex private_key string!");

        let bytes = Self::pbkdf2(&private_key, &salt);

        Self {
            bytes,
        }
    }

    pub fn new_from_string(s: &str, password: &str) -> Self {
        let salt = format!("cyfs-mnemonic-{}", password);
        
        let bytes = Self::pbkdf2(s.as_bytes(), &salt);

        Self {
            bytes,
        }
    }

    /// Get the seed value as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl AsRef<[u8]> for Seed {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#X}", self)
    }
}

impl fmt::LowerHex for Seed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            f.write_str("0x")?;
        }

        for byte in &self.bytes {
            write!(f, "{:x}", byte)?;
        }

        Ok(())
    }
}

impl fmt::UpperHex for Seed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            f.write_str("0x")?;
        }

        for byte in &self.bytes {
            write!(f, "{:X}", byte)?;
        }

        Ok(())
    }
}
