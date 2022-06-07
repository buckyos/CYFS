const child_process = require('child_process');

const env = process.env;

child_process.execSync('cargo update', { stdio: 'inherit', cwd: __dirname})
child_process.execSync('rustup target add aarch64-apple-ios x86_64-apple-ios', { stdio: 'inherit', cwd: __dirname})

// 编译通用库
child_process.execSync('cargo install cargo-lipo', { stdio: 'inherit', cwd: __dirname})
child_process.execSync('cargo lipo -p ood-control --release', { stdio: 'inherit', cwd: __dirname})

// 编译模拟器库
child_process.execSync('cargo build -p ood-control --lib --target x86_64-apple-ios --release', { stdio: 'inherit', cwd: __dirname, env: env })

// 编译真机库
child_process.execSync('cargo build -p ood-control --lib --target aarch64-apple-ios --release', { stdio: 'inherit', cwd: __dirname, env: env })

// 生成头文件
//child_process.execSync('sudo cargo install --force cbindgen', { stdio: 'inherit', cwd: __dirname})
// cbindgen 可以导出指定crate的pub方法或类型
//child_process.execSync('sudo cbindgen --config cbindgen.toml --crate ood-control --output header/ood_control.h', { stdio: 'inherit', cwd: __dirname})


