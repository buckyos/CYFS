extern crate protoc_rust;

use std::io::Write;

static MOD_PROTOS_RS:&str = r#"
mod perf_objects;
pub use perf_objects::*;
"#;

fn gen_protos() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let mut config = prost_build::Config::new();
    config.default_package_filename("perf_objects");
    config
        .compile_protos(&["protos/perf_objects.proto"], &["protos"])
        .expect("prost error!");
    std::fs::File::create(out_dir + "/mod.rs").expect("write protos mod error")
        .write_all(MOD_PROTOS_RS.as_ref()).expect("write protos mod error");
}

fn main() {
    println!("cargo:rerun-if-changed=protos");
    gen_protos();
}
