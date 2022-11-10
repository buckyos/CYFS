const child_process = require('child_process');
const fs = require('fs');
const path = require('path')
const toml = require('@ltd/j-toml')
let root;
if (process.platform === 'win32') {
	root = "c:\\cyfs"
} else if (process.platform === 'darwin') {
    root = path.join(process.env['HOME'], 'Library', 'cyfs')
} else {
	root = "/cyfs"
}
const env = process.env;

const buildnumber = process.env["VERSION"] || "0"
const channel = process.env["CHANNEL"] || "nightly"

function version_from_channel(channel) {
    if (channel === "nightly") {
        return 0
    } else if (channel === "beta") {
        return 1
    } else if (channel === "stable") {
        return 2
    } else {
        console.log("unknown channel name:", channel)
        process.exit(1);
    }
}

function read_default_target() {
	for (const line of child_process.execSync('rustc -vV', {encoding: 'utf-8'}).split('\n')) {
		if (line.startsWith("host: ")) {
			return line.substring(6)
		}
	}
}
const target = read_default_target();
const version = `1.0.${version_from_channel(channel)}.${buildnumber}`

console.log("build CYFS OOD Service")
console.log('build target:', target)
console.log("build channel:", channel)
console.log('build version:', version)

const services = ["app-manager", "file-manager", "chunk-manager", "gateway", "ood-daemon"]
const services_id = {
	"app-manager": "9tGpLNnDwJ1nReZqJgWev5eoe23ygViGDC4idnCK1Dy5",
	"file-manager": "9tGpLNnDpa8deXEk2NaWGccEu4yFQ2DrTZJPLYLT7gj4",
	"chunk-manager": "9tGpLNnabHoTxFbodTHGPZoZrS9yeEZVu83ZVeXL9uVr",
	"gateway": "9tGpLNnQnReSYJhrgrLMjz2bFoRDVKP9Dp8Crqy1bjzY",
	"ood-daemon": "9tGpLNnPtNxqwjgcxKfMpyaQqVRkLQ5aka69FgWy5PLU",
}

console.log('build tools for pack standalone ood')
child_process.execSync(`cargo build -p pack-tools -p cyfs-client -p desc-tool --release`, {stdio: "inherit"})

console.log('create random people key pair')
child_process.execSync(`${path.join("target", "release", "desc-tool")} create people --idfile people_id`)
let people = fs.readFileSync('people_id').toString();
env["VERSION"] = buildnumber;
env["CHANNEL"] = channel;

const repo_store_path = path.join(root, "repo_store");

fs.mkdirSync(repo_store_path, {recursive: true})

let ext = target.includes("windows")?".exe":'';

let device_config = {service: []}
for (const service of services) {
	console.log("build and pack service", service, "target", target)
	child_process.execSync(`cargo build -p ${service} --target ${target} --release`, {env: env, stdio:"inherit"})
	fs.mkdirSync(path.join("dist", "services", service, target, "bin"), {recursive: true})
	fs.copyFileSync(path.normalize(`target/${target}/release/${service}${ext}`), path.normalize(`dist/services/${service}/${target}/bin/${service}${ext}`))
	fs.copyFileSync(path.normalize(`service/${service}/service/linux/package.cfg`), path.normalize(`dist/services/${service}/${target}/package.cfg`))

	child_process.execSync(`${path.normalize("../target/release/pack-tools")} -d services/${service}/${target}`, { cwd: 'dist'})
	fs.rmSync(path.normalize(`dist/services/${service}/${target}`), { recursive: true, force: true })

	child_process.execSync(`${path.normalize("../target/release/cyfs-client")} create services/${service} -f fid -o ../${people}`, {cwd: 'dist'})
	let fid = fs.readFileSync(path.normalize('dist/fid')).toString();
	device_config.service.push({ id: services_id[service], name: service, ver: `${version}`, enable: true, target_state: "RUN", fid: `${fid}/${target}.zip` })

	fs.copyFileSync(path.normalize(`dist/services/${service}/${target}.zip`), path.join(repo_store_path, `${fid}_${target}.zip`))
}
// fs.writeFileSync(path.join(repo_store_path, 'device-config.cfg'), JSON.stringify(device_config))
fs.writeFileSync(path.join(repo_store_path, 'device-config.toml'), toml.stringify(device_config, {newline: '\n'}));

console.log('build ood-installer')
child_process.execSync('cargo build -p ood-installer --release')
fs.copyFileSync(path.normalize(`target/release/ood-installer${ext}`), `ood-installer${ext}`)

console.log('clean tmp file...')
fs.unlinkSync('people_id')
fs.unlinkSync(`${people}.desc`)
fs.unlinkSync(`${people}.sec`)
fs.rmSync('dist', { recursive: true, force: true })

console.log("build standalone ood env success. \nrun ./ood-installer --target solo to install your own ood")
