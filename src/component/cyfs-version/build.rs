extern crate chrono;

fn main() {
    println!("cargo:rerun-if-env-changed=VERSION");

    println!(
        "cargo:rustc-env=VERSION={}",
        std::env::var("VERSION").unwrap_or("0".to_owned())
    );

    println!(
        "cargo:rustc-env=BUILDDATE={}",
        chrono::Local::today().format("%y-%m-%d")
    );
}