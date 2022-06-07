var XMLHttpRequest = require("xmlhttprequest").XMLHttpRequest;
var process = require('process');
const fs = require('fs');
const request = require('request');
const dgram = require('dgram');
const http = require('http');
const bucky = console;
const Koa = require('koa');
const cors = require('koa-cors');
const logger = require('koa-logger');
const bodyParser = require('koa-bodyparser');
const Router = require('koa-router');
const { assert } = require('console');


function start_http_server() {
    const http = require('http');

    const requestListener = function(req, res) {
        res.writeHead(200);
        res.end('gateway test server');
    };

    const server = http.createServer(requestListener);
    server.listen(1002, "127.0.0.1");
}

async function start_http_client() {
    return new Promise((resolve) => {
        request.post(`http://127.0.0.1:81/test`, {
            //json: JSON.parse(block),
        }, (error, res, body) => {
            if (error) {
                console.error(error);
                return resolve(-1);
            }

            console.log(`statusCode: ${res.statusCode}`);
            console.log(body);

            if (res.statusCode == 200) {
                resolve(0);
            } else {
                resolve(-1);
            }
        });
    });
}

const block = `
{
    "id": "test1",
    "type": "http",
    "value":{
        "block": "server",

        "listener": [{
                "listen": "127.0.0.1:80"
            }, {
                "listen": "127.0.0.1:81"
            }
        ],

        "server_name": "192.168.100.115 127.0.0.1 www.cyfs.com",

        "location": [{
                "type": "prefix",
                "path": "/test",
                "method": "get POST",
                "proxy_pass": "127.0.0.1:1002/"
            }
        ]
    }
}
`;


const unregister_block = `
{
    "id": "test1",
    "type": "http"
}
`;

const gateway_uri = "http://127.0.0.1:9088";

function register() {
    return new Promise((resolve) => {
        request.post(`${gateway_uri}/register`, {
            json: JSON.parse(block),
        }, (error, res, body) => {
            if (error) {
                console.error(error);
                return resolve(-1);
            }
    
            console.log(`statusCode: ${res.statusCode}`);
            console.log(body);

            if (res.statusCode === 200) {
                resolve(0);
            } else {
                resolve(-1);
            }
        });
    });
}

function unregister() {
    return new Promise((resolve) => {
        request.post(`${gateway_uri}/unregister`, {
            json: JSON.parse(unregister_block),
        }, (error, res, body) => {
            if (error) {
                console.error(error);
                return resolve(-1);
            }
    
            console.log(`statusCode: ${res.statusCode}`);
            console.log(body);

            if (res.statusCode === 200) {
                resolve(0);
            } else {
                resolve(-1);
            }
        });
    });
}

(async () => {

    let ret = await register();
    if (ret != 0) {
        process.exit(ret);
    }

    start_http_server();
    ret = await start_http_client();
    if (ret != 0) {
        process.exit(ret);
    }

    //ret = await unregister();
    process.exit(ret);
})();


setInterval(() => {}, 1000);