const only_ood = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
const formal_platform = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin"]

const installer = [
    {
        "name": "ood-installer",
        "include": only_ood
    },
]

const tools = [
    {
        "name": "desc-tool",
        "include": formal_platform
    },
    {
        "name": "cyfs-client",
        "include": formal_platform
    },
    {
        "name": "cyfs-meta-client",
        "include": formal_platform
    },
    {
        "name": "app-tool-ex",
        "include": formal_platform
    },
    {
        "name": "app-tool",
        "include": ["x86_64-pc-windows-msvc"]
    },
    {
        "name": "sn-updater",
        "include": ["x86_64-pc-windows-msvc"]
    },
    {
        "name": "pack-tools",
        "include": formal_platform
    },
    {
        "name": "zone-simulator",
        "include": formal_platform
    },
]

const service_default_cfg = {
    "x86_64-pc-windows-msvc": "service/win/package.cfg",
    "i686-pc-windows-msvc": "service/win/package.cfg",
    "default": "service/linux/package.cfg"
}

const services = [
    {
        "name": "gateway",
        "include": only_ood,
        "pub": true,
        "config_file": service_default_cfg,
        "id": "9tGpLNnQnReSYJhrgrLMjz2bFoRDVKP9Dp8Crqy1bjzY",
    },
    {
        "name": "chunk-manager",
        "include": only_ood,
        "pub": true,
        "config_file": service_default_cfg,
        "id": "9tGpLNnabHoTxFbodTHGPZoZrS9yeEZVu83ZVeXL9uVr"
    },
    {
        "name": "file-manager",
        "include": only_ood,
        "pub": true,
        "config_file": service_default_cfg,
        "id": "9tGpLNnDpa8deXEk2NaWGccEu4yFQ2DrTZJPLYLT7gj4"
    },
    {
        "name": "ood-daemon",
        "include": only_ood,
        "pub": true,
        "config_file": service_default_cfg,
        "id": "9tGpLNnTdsycFPRcpBNgK1qncX6Mh8chRLK28mhNb6fU"
    },
    {
        "name": "app-manager",
        "include": only_ood,
        "pub": true,
        "config_file": service_default_cfg,
        "id": "9tGpLNnDwJ1nReZqJgWev5eoe23ygViGDC4idnCK1Dy5"
    }
]


const sdk = [
    {
        "name": "cyfs-runtime",
        "include": formal_platform,
        "icon": "",
        "lib": { "aarch64-linux-android": "cyfsruntime" },
    },
    {
        "name": "cyfs-mobile-stack",
        "include": ["aarch64-linux-android", "armv7-linux-androideabi"],
        "lib": { "aarch64-linux-android": "cyfsstack", "armv7-linux-androideabi": "cyfsstack" },
    }
]

const metas = [
    {
        "name": "cyfs-meta-genesis",
        "include": formal_platform
    },
    {
        "name": "cyfs-meta-miner",
        "include": formal_platform
    },
	{
        "name": "cyfs-meta-spv",
        "include": formal_platform
    },
    {
        "name": "browser-meta-spv",
        "include": formal_platform
    },
    {
        "name": "meta-stat",
        "include": formal_platform
    },
]

const misc = [
    {
        "name": "sn-miner-rust",
        "include": formal_platform
    },
    {
        "name": "cyfs-monitor",
        "include": formal_platform
    },
    {
        "name": "sn-online-test",
        "include": formal_platform
    },
]

module.exports = {
    tools,
    services,
    sdk,
    installer,
    metas,
    misc,
    step_file: "curstep"
}