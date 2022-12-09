extern crate protoc_rust;
extern crate chrono;

use std::io::Write;

static MOD_PROTOS_RS:&str = r#"
pub(crate) mod group_bft_protocol;

pub(crate) use group_bft_protocol::*;
"#;

#[allow(dead_code)]
fn gen_protos() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let mut gen = protoc_rust::Codegen::new();
    gen.input("protos/group_bft_protocol.proto")
        .protoc_path(protoc_bin_vendored::protoc_bin_path().unwrap())
        .out_dir(&out_dir)
        .customize(protoc_rust::Customize {
            expose_fields: Some(true),
            ..Default::default()
        });

    gen.run().expect("protoc error!");

    std::fs::File::create(out_dir + "/mod.rs").expect("write protos mod error")
        .write_all(MOD_PROTOS_RS.as_ref()).expect("write protos mod error");
}

fn main() {
    println!("cargo:rerun-if-env-changed=VERSION");
    println!("cargo:rerun-if-env-changed=CHANNEL");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!(
        "cargo:rustc-env=VERSION={}",
        std::env::var("VERSION").unwrap_or("0".to_owned())
    );
    println!(
        "cargo:rustc-env=CHANNEL={}",
        std::env::var("CHANNEL").unwrap_or("nightly".to_owned())
    );
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );

    println!(
        "cargo:rustc-env=BUILDDATE={}",
        chrono::Local::today().format("%y-%m-%d")
    );

    println!("cargo:rerun-if-changed=protos");

    gen_protos();
}
