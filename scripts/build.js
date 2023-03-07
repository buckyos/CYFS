const fs = require('fs')
const build_util = require('./build_util')
const build_config = require('./build_config')

// build_util.reflesh_cargo();

// 非ios: x86_64-pc-windows-msvc;x86_64-unknown-linux-gnu;aarch64-linux-android;i686-pc-windows-msvc;armv7-linux-androideabi
// ios: aarch64-apple-ios

const targets = process.argv[2].split(";")
const types = process.argv[3].split(";")
const buildnumber = process.argv[4] || "0"
const channel = process.argv[5] || "nightly"
const buildType = process.argv[6] || "release"

if (!fs.existsSync('Cargo.toml')) {
    console.error('cannot find Cargo.toml in cwd! check working dir')
}

function build(catalogy, need_pack, need_bin) {
    fs.rmSync(build_config.step_file, {force: true, maxRetries: 3})
    fs.rmSync(`dist/${catalogy}`, {recursive: true, force: true})
    if (build_config[catalogy] === undefined) {
        console.error(`build catalogy ${catalogy} not exists in config`)
        return
    }

    for (const prog of build_config[catalogy]) {
        for (const target of targets) {
            let prog_name = build_util.build(prog, buildType, target, buildnumber, channel, "~/workspace/ffs")
            if (prog_name === undefined) {
                continue
            }
            let target_dir
            if (need_pack) {
                target_dir = `dist/${catalogy}/${prog.name}/${target}`
                if (need_bin) {
                    target_dir = `${target_dir}/bin`
                }
            } else {
                target_dir = `dist/${catalogy}/${target}`
            }
            if (!fs.existsSync(target_dir)) {
                fs.mkdirSync(target_dir, {recursive: true})
            }
            fs.copyFileSync(`target/${target}/${buildType}/${prog_name}`, `${target_dir}/${prog_name}`);
        }
    }
}

for (const type of types) {
    let need_pack = type === "services" || type === "apps"
    let need_bin = type === "services"
    build(type, need_pack, need_bin)
}