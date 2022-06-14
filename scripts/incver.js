const {publish_packages} = require('./cargo_config')
const path = require('path')
const fs = require('fs')
const child_process = require('child_process')
const toml = require('@ltd/j-toml')
const semver = require('semver')

if (!fs.existsSync('Cargo.toml')) {
    console.error('script MUST RUN IN src dir!')
    process.exit(1)
}

const metadatas = JSON.parse(child_process.execSync('cargo metadata --no-deps --offline', {encoding: 'utf-8'}))
const last_cargo_rev = fs.readFileSync('../cargo_pub_rev', {encoding: 'utf-8'});
// 遍历所有工程
for (const metadata of metadatas.packages) {
    if (!publish_packages.includes(metadata.name)) {
        continue
    }

    let project_path = path.dirname(metadata.manifest_path)
    let changes = child_process.execSync(`git log --oneline ${last_cargo_rev}..HEAD ${project_path}`);
    if (changes.length > 0) {
        console.log(`project ${metadata.name} has commits`)
        let project_toml = toml.parse(fs.readFileSync(metadata.manifest_path, "utf-8"), {x: {comment: true}});
        let newver = semver.inc(project_toml.package.version, 'patch')
        console.log(`project ${metadata.name} ver ${project_toml.package.version} => ${newver}`)
        project_toml.package.version = newver;
        fs.writeFileSync(metadata.manifest_path, toml.stringify(project_toml, {newline: "\n", newlineAround: "section", }))
    }
}