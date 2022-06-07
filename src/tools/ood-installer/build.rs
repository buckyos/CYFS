fn main() {
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    println!("cargo:rerun-if-env-changed=VERSION");
    println!(
        "cargo:rustc-env=VERSION={}",
        std::env::var("VERSION").unwrap_or("1.0.0.0".to_owned())
    );
}