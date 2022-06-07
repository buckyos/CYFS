const only_ood = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", 'aarch64-unknown-linux-gnu']
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
        "name": "pack-tools",
        "include": formal_platform
    },
    {
        "name": "zone-simulator",
        "include": formal_platform
    },
    {
        "name": "net-tool",
        "include": only_ood
    },
    {
        "name": "cyfs-monitor",
        "include": ["x86_64-unknown-linux-gnu"]
    },
]

const apps = [
    {
        "name": "sn-miner-rust",
        "include": formal_platform,
        "pub": false,
        "config_file": {
            "default": "config/package.cfg"
        }
    },
    {
        "name": "pn-miner",
        "include": formal_platform,
        "pub": false,
        "config_file": {
            "default": "config/package.cfg"
        }
    },
    {
        "name": "perf-service",
        "include": formal_platform,
        "pub": false,
        "config_file": {
            "default": "config/package.cfg"
        }
    },
    {
        "name": "cyfs-monitor-daemon",
        "include": formal_platform,
        "pub": false,
        "config_file": {
            "default": "service/package.cfg"
        }
    }
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
        "assets": {
            "x86_64-pc-windows-msvc": [
                {
                    from: "../3rd/windivert/x64/WinDivert.dll",
                    to: "bin/WinDivert.dll"
                },
                {
                    from: "../3rd/windivert/x64/WinDivert64.sys",
                    to: "bin/WinDivert64.sys"
                }
            ],
            "i686-pc-windows-msvc": [
                {
                    from: "../3rd/windivert/x86/WinDivert.dll",
                    to: "bin/WinDivert.dll"
                },
                {
                    from: "../3rd/windivert/x86/WinDivert32.sys",
                    to: "bin/WinDivert64.sys"
                }
            ]
        }
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
    },
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
        "name": "cyfs-meta-tool",
        "include": formal_platform
    },
    {
        "name": "browser-meta-spv",
        "include": formal_platform
    },
]

const tests = [
    {
        "name": "raptor-test"
    },
    {
        "name": "rust-bdt-test-demo"
    }
]

const sdk = [
    {
        "name": "cyfs-runtime",
        "include": formal_platform.concat(["aarch64-linux-android"]),
        "icon": "",
        "lib": { "aarch64-linux-android": "cyfsruntime" },
    },
    {
        "name": "mobile-stack",
        "include": ["aarch64-linux-android", "armv7-linux-androideabi"],
        "lib": { "aarch64-linux-android": "cyfsstack", "armv7-linux-androideabi": "cyfsstack" },
    },
    {
        "name": "ood-control",
        "include": ["aarch64-linux-android", "armv7-linux-androideabi"],
        "lib": { "aarch64-linux-android": "ood_control", "armv7-linux-androideabi": "ood_control" },
    }
]

const dsg = [
    {
        "name": "dsg-meta-miner",
        "include": formal_platform
    },
    {
        "name": "dsg-meta-spv",
        "include": formal_platform
    }
]

module.exports = {
    tools,
    apps,
    services,
    metas,
    tests,
    sdk,
    dsg,
    installer
}