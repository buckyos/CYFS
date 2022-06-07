extern crate protoc_rust;

fn gen_protos() {
    let mut gen = protoc_rust::Codegen::new();
    gen.input("protos/contracts.proto")
    .input("protos/proof.proto")
    .protoc_path(protoc_bin_vendored::protoc_bin_path().unwrap())
        .out_dir("src/protos")
        .customize(protoc_rust::Customize {
            expose_fields: Some(true),
            ..Default::default()
        });

    gen.run().expect("protoc error!");
}

fn main() {
    println!("cargo:rerun-if-changed=protos");
    gen_protos();
}
