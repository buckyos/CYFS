fn main() {
    println!("cargo:rerun-if-env-changed=CYFS_MONITOR_DINGTOKEN");

    if let Ok(var) = std::env::var("CYFS_MONITOR_DINGTOKEN") {
        println!(
            "cargo:rustc-env=DINGTOKEN={}",
            var
        );
    }

}