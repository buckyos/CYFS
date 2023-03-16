use std::io::Write;

static MOD_PROTOS_RS: &str = r#"
pub mod backup_objects;
mod backup_objects_with_macro;

pub use backup_objects_with_macro::*;
"#;

fn gen_protos() {
    let mut gen = protoc_rust::Codegen::new();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    gen.input("protos/backup_objects.proto")
    .protoc_path(protoc_bin_vendored::protoc_bin_path().unwrap())
        .out_dir(&out_dir)
        .customize(protoc_rust::Customize {
            expose_fields: Some(true),
            ..Default::default()
        });

    gen.run().expect("protoc backup_objects error!");

    let mut config = prost_build::Config::new();
    config.default_package_filename("backup_objects_with_macro");
    config
        .compile_protos(&["protos/backup_objects.proto"], &["protos"])
        .unwrap();

    std::fs::File::create(out_dir + "/mod.rs").expect("write protos mod error")
        .write_all(MOD_PROTOS_RS.as_ref()).expect("write protos mod error");
}


fn main() {
    println!("cargo:rerun-if-changed=protos");
    println!("cargo:warning={}", format!("cyfs-backup run build script, OUT_DIR={}", std::env::var("OUT_DIR").unwrap()));
    gen_protos();
}
