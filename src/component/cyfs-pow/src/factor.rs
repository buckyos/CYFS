use cyfs_base::*;

fn run_case(sk: PrivateKey) -> u64 {
    println!("will run case: {}", sk.key_type());
    
    let hash = hash_data("test".as_bytes());
    let begin = std::time::Instant::now();
    for _ in 0..10000 {
        sk.sign_data_hash(&hash).unwrap();
    }
    let end = begin.elapsed().as_secs();
    end
}

pub fn run() {
    let sk = PrivateKey::generate_rsa(1024).unwrap();
    let t1 = run_case(sk);
    println!("rsa1024: {}, factor={}", t1, 1000);

    let sk = PrivateKey::generate_rsa(2048).unwrap();
    let t2 = run_case(sk);
    println!("rsa2048: {}, factor={}", t2, t2 * 1000 / t1);

    let sk = PrivateKey::generate_rsa(3072).unwrap();
    let t3 = run_case(sk);
    println!("rsa3072: {}, factor={}", t3, t3 * 1000 / t1);

    let sk = PrivateKey::generate_secp256k1().unwrap();
    let t4 = run_case(sk);
    println!("secp256k1: {}, factor={}", t4, t4 * 1000 / t1);
}