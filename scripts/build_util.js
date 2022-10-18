const fs = require('fs')
const child_process = require('child_process');

const env = process.env;
const ndk_home = "C:\\Users\\vip\\AppData\\Local\\Android\\Sdk\\ndk\\21.0.6113669"
env["AR_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-ar.exe`
env["CC_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android21-clang.cmd`
env["CARGO_HTTP_MULTIPLEXING"] = "false"
const aarch64_linux_android_objcopy = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-objcopy.exe`

function reflesh_cargo() {
    try {
        fs.rmSync('cargo.lock');
    } catch (error) {}

    // 先用固定版本代替
    // fs.copyFileSync("Cargo.lock.base", "Cargo.lock")
    child_process.execSync('cargo update', { stdio: 'inherit', cwd: __dirname})
}

function build(prog, buildType, target, version, channel) {
    env["VERSION"] = version;
    env["CHANNEL"] = channel;
    if (prog.exclude && prog.exclude.includes(target)) {
        return
    }
    if (prog.include && !prog.include.includes(target)) {
        return
    }
    let cmd = `cargo build -p ${prog.name} --target=${target}`;
    if (buildType === "release") {
        cmd += " --release"
    }
    let bin_name = prog.name;
    let ext = '';
    if (target.includes("windows")) {
        ext = ".exe"
    }
    if (prog.lib && prog.lib[target]) {
        bin_name = `lib${prog.lib[target]}`
        ext = ".so"
    }
    if (target.includes("unknown-linux")) {
        cmd = cmd.replace('cargo','~/.cargo/bin/cargo');
        cmd = `bash -c "export CARGO_HTTP_MULTIPLEXING=false;export VERSION=${version};export CHANNEL=${channel};${cmd}"`
    }
    child_process.execSync(cmd, { stdio: 'inherit' })
    if (target.includes("unknown-linux")) {
        // split exe and debug info
        
        let cmd = 'objcopy'
        if (target === 'aarch64-unknown-linux-gnu') {
            cmd = aarch64_linux_android_objcopy;
        }
        child_process.execSync(`bash -c "${cmd} --only-keep-debug target/${target}/${buildType}/${bin_name}${ext} target/${target}/${buildType}/${bin_name}${ext}.debug"`)
        child_process.execSync(`bash -c "${cmd} --strip-debug --strip-unneeded target/${target}/${buildType}/${bin_name}${ext}"`)
    }
    return `${bin_name}${ext}`
}

module.exports = {
    reflesh_cargo,
    build
}