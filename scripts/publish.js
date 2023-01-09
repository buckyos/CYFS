const toml = require('@ltd/j-toml')
const fs = require('fs')
const child_process = require('child_process')

const {publish_packages} = require('./cargo_config')

if (!fs.existsSync('Cargo.toml')) {
    console.error('script MUST RUN IN src dir!')
    process.exit(1)
}

const metadatas = JSON.parse(child_process.execSync('cargo metadata --no-deps --offline', {encoding: 'utf-8'}))

function get_remote_version(name) {
    let out = child_process.execSync(`cargo search ${name}`, {encoding:'utf-8'})
    return toml.parse(out)[name]
}

let rev = child_process.execSync(`git rev-parse --short=8 HEAD`, {encoding:'utf-8'}).trim();
fs.writeFileSync('../cargo_pub_rev', rev)
console.log('write rev', rev)

// 遍历所有工程

function find_metadata(package, packages) {
    return packages.find((value) => {
        return value.name === package
    });
}

for (const package of publish_packages) {
    let metadata = find_metadata(package, metadatas.packages);

    const local_version = metadata.version
    const remote_version = get_remote_version(metadata.name)
    if (local_version !== remote_version) {
        console.log(`found ${metadata.name} local ${local_version}, remote ${remote_version}, need publish?`)
        let ret = child_process.spawnSync(`cargo publish -p ${metadata.name} --allow-dirty --no-verify`, {stdio: 'inherit', shell: true})
        if (ret.status !== 0) {
            console.log(`publish package ${metadata.name} failed, please retry.`)
            process.exit(0)
        }

        child_process.spawnSync('timeout /T 3 /NOBREAK', {stdio: 'inherit', shell: true})
    }
}