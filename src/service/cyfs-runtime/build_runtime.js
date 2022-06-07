const child_process = require('child_process');

const env = process.env;
const ndk_home = "C:\\Users\\vip\\AppData\\Local\\Android\\Sdk\\ndk\\21.0.6113669"

env["AR_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-ar.exe`
env["CC_aarch64-linux-android"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android21-clang.cmd`
env["AR_armv7-linux-androideabi"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\arm-linux-androideabi-ar.exe`
env["CC_armv7-linux-androideabi"] = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\armv7a-linux-androideabi21-clang.cmd`
const aarch64_linux_android_strip = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\aarch64-linux-android-strip.exe`
const armv7_linux_androideabi_strip = `${ndk_home}\\toolchains\\llvm\\prebuilt\\windows-x86_64\\bin\\arm-linux-androideabi-strip.exe`

child_process.execSync('cargo update', { stdio: 'inherit', cwd: __dirname})

child_process.execSync('cargo build -p cyfs-runtime --lib --target aarch64-linux-android --release', { stdio: 'inherit', cwd: __dirname, env: env })
// child_process.execSync('cargo build -p cyfs-lib --lib --target aarch64-linux-android --release', { stdio: 'inherit', cwd: __dirname, env: env })

// child_process.execSync('cargo build -p cyfs-runtime --lib --target armv7-linux-androideabi --release', { stdio: 'inherit', cwd: __dirname, env: env })
// child_process.execSync('cargo build -p cyfs-lib --lib --target armv7-linux-androideabi --release', { stdio: 'inherit', cwd: __dirname, env: env })

// child_process.execSync(`${aarch64_linux_android_strip} target/aarch64-linux-android/release/libimclient.so`)
