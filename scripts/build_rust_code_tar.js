const tar = require('tar')
const fs = require('fs');

function getCurrentBuild() {
    const execSync = require('child_process').execSync;
    const code = execSync('git rev-list --count --first-parent HEAD', {
        encoding: 'utf8'
    }).trim();

    return parseInt(code);
}

const pack_files = [];
const files = fs.readdirSync("../src");
for (const file of files)
{
    const path = `../src/${file}`;
    if (fs.lstatSync(path).isFile())
    {
        console.log("file: ", path);
        pack_files.push(path);
    }
}

const pack_dirs = [
    '../src/.cargo',
    '../src/3rd',
    '../src/component',
    '../src/service',
    '../src/tools',
];
for (dir of pack_dirs)
{
    console.log("dir: ", dir);
}

const all = pack_files.concat(pack_dirs);

const build_dir = `../build`;
if (!fs.existsSync(build_dir)){
    fs.mkdirSync(build_dir);
}

const build = getCurrentBuild();
const target = `${build_dir}/cyfs-rust-src.r${build}.tgz`;
tar.c(
       {
           gzip : 'czf',
           file : target,
       },
       all,
       )
    .then(_ => {
        console.info(`pack complete: ${target}`);
        process.exit(0);
    });