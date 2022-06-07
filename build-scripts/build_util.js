const fs = require('fs')
const child_process = require('child_process');

const env = process.env;
const ndk_home = "C:\\Users\\vip\\AppData\\Local\\Android\\Sdk\\ndk\\21.0.6113669"
const openwrt_toolchain = "/mnt/c/buildtools/openwrt-toolchain-x86-64_gcc-7.3.0_musl.Linux-x86_64/toolchain-x86_64_gcc-7.3.0_musl"
const aarch64_linux = "/mnt/c/buildtools/aarch64-unknown-linux-gnueabi"
env["AR_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-ar.exe`
env["CC_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android21-clang.cmd`
env["AR_armv7-linux-androideabi"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\arm-linux-androideabi-ar.exe`
env["CC_armv7-linux-androideabi"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\armv7a-linux-androideabi21-clang.cmd`
env["CC_x86_64-unknown-linux-musl"] = `${openwrt_toolchain}/bin/x86_64-openwrt-linux-musl-cc`
env["AR_x86_64-unknown-linux-musl"] = `${openwrt_toolchain}/bin/x86_64-openwrt-linux-musl-ar`
env["CC_aarch64_unknown_linux_gnu"] = `${aarch64_linux}/bin/aarch64-unknown-linux-gnueabi-cc`
env["AR_aarch64_unknown_linux_gnu"] = `${aarch64_linux}/bin/aarch64-unknown-linux-gnueabi-ar`
env["CARGO_HTTP_MULTIPLEXING"] = "false"
const aarch64_linux_android_strip = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-strip.exe`
const armv7_linux_androideabi_strip = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\arm-linux-androideabi-strip.exe`
const aarch64_unknown_linux_gnu_objcopy = `${aarch64_linux}/bin/aarch64-unknown-linux-gnueabi-objcopy`
const aarch64_unknown_linux_gnu_strip = `${aarch64_linux}/bin/aarch64-unknown-linux-gnueabi-strip`

function reflesh_cargo() {
    try {
        fs.rmSync('cargo.lock');
    } catch (error) {}

    // 先用固定版本代替
    // fs.copyFileSync("Cargo.lock.base", "Cargo.lock")
    child_process.execSync('cargo update', { stdio: 'inherit', cwd: __dirname})
}

function build(prog, buildType, target, version, channel, bash_base_path) {
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
        cmd = `bash -c "export CARGO_HTTP_MULTIPLEXING=false;export VERSION=${version};export CHANNEL=${channel};cd ${bash_base_path};${cmd}"`
    }
    child_process.execSync(cmd, { stdio: 'inherit', cwd: __dirname, env: env })
    if (target.includes("unknown-linux")) {
        let cmd = 'strip'
        if (target === 'aarch64-unknown-linux-gnu') {
            cmd = aarch64_unknown_linux_gnu_strip;
        }
        // child_process.execSync(`bash -c "cd ${bash_base_path};${cmd} target/${target}/${buildType}/${bin_name}${ext}"`)
        // 这里要拷贝编译后的文件到原来的目录
        child_process.execSync(`bash -c "mkdir -p target/${target}/${buildType}"`)
        child_process.execSync(`bash -c "cp -f ${bash_base_path}/target/${target}/${buildType}/${bin_name}${ext} target/${target}/${buildType}/${bin_name}${ext}"`)
    }
    /*
    if (target === "aarch64-linux-android") {
        child_process.execSync(`${aarch64_linux_android_strip} target/${target}/${buildType}/${bin_name}${ext}`)
    }
    if (target === "armv7-linux-androideabi") {
        child_process.execSync(`${armv7_linux_androideabi_strip} target/${target}/${buildType}/${bin_name}${ext}`)
    }
     */
    return `${bin_name}${ext}`
}

module.exports = {
    reflesh_cargo,
    build
}