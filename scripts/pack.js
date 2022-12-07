const fs = require('fs')
const child_process = require('child_process');
const targets = process.argv[2].split(";")
const type = process.argv[3].split(";")
const { apps, services } = require('./build_config')
const path = require('path')

const onlyput = process.argv[4] || "put"
const action = process.argv[5] || "put"
const buildnumber = process.argv[6] || "0"
const channel = process.argv[7] || "nightly"

if (!fs.existsSync('Cargo.toml')) {
    console.error('cannot find Cargo.toml in cwd! check working dir')
}


const protocols = {
    'http:': require('http'),
    'https:': require('https')
}
const ding_url = `https://oapi.dingtalk.com/robot/send?access_token=${process.env.DING_TOKEN}`

async function post(url, body) {
    let url_obj = new URL(url)
    
    return new Promise((reslove, reject) => {
        let req = protocols[url_obj.protocol].request(url, {method: 'POST'}, (resp) => {
            let resp_body = "";
            resp.on('data', (chunk) => {
                resp_body += chunk;
            });
            resp.on('end', () => {
                reslove(resp_body)
            })
        });
        
        if (body) {
            if (typeof body === "object") {
                body = JSON.stringify(body)
                req.setHeader('Content-Type', 'application/json')
            }
            req.write(body);
        }
        
        req.end()
    })
}

async function send_msg(msg) {
    let body = {msgtype: 'text', text: {content:'提醒：'+msg}}
    await post(ding_url, body)
}

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

function meta_url(channel) {
    if (channel === "nightly") {
        return 'http://nightly.meta.cyfs.com:1423'
    } else if (channel === "beta") {
        return "http://beta.meta.cyfs.com:1423";
    } else if (channel === "stable") {
        return ""
    } else {
        console.log("unknown channel name:", channel)
        process.exit(1);
    }
}

let version = `1.0.${version_from_channel(channel)}.${buildnumber}`;

let repo_path = process.env.FFS_SERVICE_REPO_DESC;
if (!repo_path) {
    console.error('no service repo desc path, please set env FFS_SERVICE_REPO_DESC')
    process.exit(1)
}

let file_repo_path = process.env.FFS_SERVICE_FILE_REPO_DESC;
if (!file_repo_path) {
    file_repo_path = repo_path
    console.warn('no service file repo desc path, set to the same as FFS_SERVICE_REPO_DESC')
}

function get_obj_id(desc_file) {
    let out = child_process.execSync(`${path.join('dist', 'desc-tool')} show ${desc_file}`, {encoding: 'utf8'})
    let obj_id
    for (const line of out.split('\n')) {
        if (line.startsWith('objectid:')) {
            obj_id = line.substring(10)
            break;
        }
    }
    return obj_id
}

async function run() {
    // check balance
    let repo_id = get_obj_id(repo_path+".desc")
    let out = JSON.parse(await post(meta_url(channel)+"/balance", [[0, repo_id]]));
    let balance = BigInt(out.result[0])
    if (balance < 10000) {
        let msg = `repo account ${repo_id} balance ${balance} less then 10000!! channel ${channel}`;
        await send_msg(msg)
        console.error(msg)
        process.exit(1)
    }
    console.log(`get repo account ${repo_id} balance ${balance}`)

    // check file repo balance
    let file_repo_id = get_obj_id(file_repo_path+".desc")
    out = JSON.parse(await post(meta_url(channel)+"/balance", [[0, file_repo_id]]));
    balance = BigInt(out.result[0])
    if (balance < 10000) {
        let msg = `file repo account ${file_repo_id} balance ${balance} less then 10000!! channel ${channel}`;
        await send_msg(msg)
        console.error(msg)
        process.exit(1)
    }
    console.log(`get file repo account ${file_repo_id} balance ${balance}`)
    
    if (type.includes("apps")) {
        try { fs.rmSync('dist/app_config.cfg') } catch (error) { }
        let app_config = []
    
        for (const app of apps) {
            if (!app.pub) {
                continue
            }
            if (onlyput !== "onlyput") {
                for (const target of targets) {
                    if (app.exclude && app.exclude.includes(target)) {
                        continue
                    }
                    if (app.include && !app.include.includes(target)) {
                        continue
                    }
                    let project_path = app.path || `app/${app.name}`
                    let config_path = app.config_file[target] || app.config_file.default
                    fs.copyFileSync(`${project_path}/${config_path}`, `dist/apps/${app.name}/${target}/package.cfg`)
    
                    if (app.assets && app.assets[target]) {
                        for (const asset of app.assets[target]) {
                            fs.copyFileSync(asset.from, `dist/apps/${app.name}/${target}/${asset.to}`)
                        }
                    }
                }
    
                child_process.execSync(`bash -c "./pack-tools -d apps/${app.name}"`, { cwd: 'dist', stdio: 'inherit' })
            }
    
            child_process.execSync(`cyfs-client ${action} apps/${app.name}.zip -f fid -o ${file_repo_path}`, { cwd: 'dist', stdio: 'inherit' })
            let fid = fs.readFileSync('dist/fid', {encoding: 'utf-8'});
            app_config.apps.push({ "id": app.appid, "ver": `${version}`, "status": 1 })
    
            // 运行app-tool，添加版本和fid
            if (app.appid !== undefined) {
                let cmd = `app-tool app set -v ${version} -s ${fid} ${app.appid} -o ${repo_path}`;
                console.log("will run app tool cmd:", cmd)
                child_process.execSync(cmd, { cwd: 'dist', stdio: 'inherit' })
            }
    
        }
    
        fs.writeJSONSync('dist/app_config.cfg', app_config)
    }
    
    
    if (type.includes("services")) {
        try { fs.removeSync('dist/device_config.cfg') } catch (error) { }
    
        let device_config = [];
        for (const service of services) {
            if (!service.pub) {
                continue
            }
            if (onlyput !== "onlyput") {
                for (const target of targets) {
                    if (service.exclude && service.exclude.includes(target)) {
                        continue
                    }
                    if (service.include && !service.include.includes(target)) {
                        continue
                    }
    
                    let config_path = service.config_file[target] || service.config_file.default
                    fs.copyFileSync(`service/${service.name}/${config_path}`, `dist/services/${service.name}/${target}/package.cfg`)
    
                    if (service.assets && service.assets[target]) {
                        for (const asset of service.assets[target]) {
                            fs.copyFileSync(asset.from, `dist/services/${service.name}/${target}/${asset.to}`)
                        }
                    }
    
                    child_process.execSync(`bash -c "./pack-tools -d services/${service.name}/${target}"`, { cwd: 'dist', stdio: 'inherit' })
                    fs.rmSync(`dist/services/${service.name}/${target}`, {recursive: true, force: true});
                }
            }
    
            child_process.execSync(`cyfs-client ${action} services/${service.name} -f fid -o ${file_repo_path} --tcp`, { cwd: 'dist', stdio: 'inherit' })
            let fid = fs.readFileSync('dist/fid', {encoding: 'utf-8'})
            device_config.push({ "id": service.id, "ver": `${version}`, "status": 1 })
    
            // 运行app-tool，添加版本和fid
            if (service.id !== undefined) {
                let cmd = `app-tool app set -v ${version} -s ${fid} ${service.id} -o ${repo_path}`;
                console.log("will run app tool cmd:", cmd)
                child_process.execSync(cmd, { cwd: 'dist', stdio: 'inherit' })
            }
        }
    
        fs.writeFileSync('dist/device-config.cfg', JSON.stringify(device_config))
    
        await send_msg(`service pub complete: ver ${version}, channel ${channel}`)
    }
}

run().then(() => {
    process.exit(0)
})

