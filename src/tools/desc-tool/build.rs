fn main() {
    println!("cargo:rerun-if-env-changed=VERSION");
    println!(
        "cargo:rustc-env=VERSION={}",
        std::env::var("VERSION").unwrap_or("1.0.0.0".to_owned())
    );
}

