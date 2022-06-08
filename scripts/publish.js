const path = require('path')
const toml = require('@ltd/j-toml')
const fs = require('fs')
const child_process = require('child_process')

const publish_packages = [
    "cyfs-base",
    "cyfs-base-derive",
    "cyfs-base-meta",
    "cyfs-bdt",
    "cyfs-chunk-lib",
    "cyfs-core",
    "cyfs-debug",
    "cyfs-ecies",
    "cyfs-lib",
    "cyfs-meta-lib",
    "cyfs-perf-base",
    "cyfs-perf-client",
    "cyfs-raptorq",
    "cyfs-sha2",
    "cyfs-task-manager",
    "cyfs-util",

]

if (!fs.existsSync('Cargo.toml')) {
    console.error('script MUST RUN IN src dir!')
    process.exit(1)
}

const metadatas = JSON.parse(child_process.execSync('cargo metadata --no-deps --offline', {encoding: 'utf-8'}))

function get_remote_version(name) {
    let out = child_process.execSync(`cargo search ${name}`, {encoding:'utf-8'})
    return toml.parse(out)[name]
}

// 遍历所有工程
for (const metadata of metadatas.packages) {
    if (!publish_packages.includes(metadata.name)) {
        continue
    }

    const local_version = metadata.version
    const remote_version = get_remote_version(metadata.name)
    if (local_version !== remote_version) {
        console.log(`found ${metadata.name} local ${local_version}, remote ${remote_version}, need publish?`)
        let ret = child_process.spawnSync(`cargo publish -p ${metadata.name}`)
        if (ret.status !== 0) {
            console.log(`publish package ${metadata.name} failed, please retry.`)
            process.exit(0)
        }
    }
}