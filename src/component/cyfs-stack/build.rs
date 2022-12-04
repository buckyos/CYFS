use protoc_bin_vendored::protoc_bin_path;

fn main() {
    println!("cargo:rerun-if-changed=protos");

    println!("cargo:rerun-if-env-changed=BUILD_NUMBER");

    println!(
        "cargo:rustc-env=BUILD_NUMBER={}",
        std::env::var("BUILD_NUMBER").unwrap_or("0".to_owned())
    );
    println!("cargo:warning={}", format!("cyfs-stack run build script, OUT_DIR={}", std::env::var("OUT_DIR").unwrap()));

    std::env::set_var("PROTOC", protoc_bin_path().unwrap().to_string_lossy().to_string());

    let mut config = prost_build::Config::new();
    config.default_package_filename("trans_proto");
    // config.out_dir("src/trans_api/local");
    config.compile_protos(&["protos/trans.proto"],
                          &["protos"]).unwrap();

    let mut config = prost_build::Config::new();
    config.default_package_filename("util_proto");
    // config.out_dir("src/util_api/local");
    config.compile_protos(&["protos/util.proto"],
                          &["protos"]).unwrap();
}
