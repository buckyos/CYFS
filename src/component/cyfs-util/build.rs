use std::io::Read;
use hex;

fn main() {
    println!("cargo:rerun-if-changed=peers/nightly-sn.desc");
    println!("cargo:rerun-if-changed=peers/beta-sn.desc");
    println!("cargo:rerun-if-changed=peers/known-peer.desc");

    let mut nightly_sn_raw = vec![];
    std::fs::File::open("peers/nightly-sn.desc").unwrap().read_to_end(&mut nightly_sn_raw).unwrap();
    println!("cargo:rustc-env=NIGHTLY_SN_RAW={}", hex::encode(&nightly_sn_raw));

    let mut beta_sn_raw = vec![];
    std::fs::File::open("peers/beta-sn.desc").unwrap().read_to_end(&mut beta_sn_raw).unwrap();
    println!("cargo:rustc-env=BETA_SN_RAW={}", hex::encode(&beta_sn_raw));

    let mut known_peer_raw = vec![];
    std::fs::File::open("peers/known-peer.desc").unwrap().read_to_end(&mut known_peer_raw).unwrap();
    println!("cargo:rustc-env=KNOWN_RAW={}", hex::encode(&known_peer_raw));
}