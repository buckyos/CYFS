const child_process = require('child_process');

const env = process.env;

const USER_HOME = env.HOME || env.USERPROFILE

// const ndk_home = "C:\\Users\\Bucky\\AppData\\Local\\Android\\Sdk\\ndk-bundle";
const ndk_home = `${USER_HOME}\\AppData\\Local\\Android\\Sdk\\ndk-bundle`

env["AR_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-ar.exe`
env["CC_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android21-clang.cmd`
env["AR_armv7-linux-androideabi"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\arm-linux-androideabi-ar.exe`
env["CC_armv7-linux-androideabi"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\armv7a-linux-androideabi21-clang.cmd`
const aarch64_linux_android_strip = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-strip.exe`
const armv7_linux_androideabi_strip = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\arm-linux-androideabi-strip.exe`

child_process.execSync('cargo update', { stdio: 'inherit', cwd: __dirname})

child_process.execSync('cargo build -p ood-control --lib --target aarch64-linux-android --release', { stdio: 'inherit', cwd: __dirname, env: env })

// child_process.execSync(`${aarch64_linux_android_strip} target/aarch64-linux-android/release/libimclient.so`)