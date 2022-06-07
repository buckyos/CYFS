mod bip32;
mod bip44;
mod seed_key;
mod path;
mod pbkdf2_rand;
mod seed;

pub use seed_key::*;
pub use path::*;

#[macro_use]
extern crate log;


#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;
    
    fn is_equal(left: &PrivateKey, right: &PrivateKey) -> bool {
        left.to_vec().unwrap() == right.to_vec().unwrap()
    }

    
    #[test]
    fn main() {
        let phrase = "bar cinnamon grow hungry lens danger treat artist hello seminar document gasp";
        let gen = CyfsSeedKeyBip::from_mnemonic(phrase, None).unwrap();
    
        // 创建people，使用mnemonic+network+address_index 替代 privateKey
        let path = CyfsChainBipPath::new_people(
            Some(CyfsChainNetwork::Main),
            Some(0),
        );
    
        println!("path={}", path.to_string());
        let key1 = gen.sub_key(&path).unwrap();
        let key2 = gen.sub_key(&path).unwrap();
        assert!(is_equal(&key1, &key2));
    
        let key1 = hex::encode(&key1.to_vec().unwrap());
        let device_gen = CyfsSeedKeyBip::from_private_key(&key1, "xxx").unwrap();
        // 创建device，使用mnemonic+network+account+address_index 替代 privateKey
        let path = CyfsChainBipPath::new_device(
            0,
            Some(CyfsChainNetwork::Main),
            Some(0),
        );
    
        println!("path={}", path.to_string());
        let key1 = device_gen.sub_key(&path).unwrap();
        let key2 = device_gen.sub_key(&path).unwrap();
        assert!(is_equal(&key1, &key2));
    
        println!("Hello, world!");
    }
    
}