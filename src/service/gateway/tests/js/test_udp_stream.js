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


function run_udp_client(port, begin) {
    let sock = dgram.createSocket("udp4");

    sock.on('error', (err) => {
        console.log(`client error:\n${err.stack}`);
        sock.close();
    });

    let index = begin;
    let send = () => {
        const msg = `${begin}: msg from client: index=${index++}`;
        sock.send(msg, port, "127.0.0.1");
    };

    sock.on('message', (msg, rinfo) => {
        console.log(`client got: ${msg} from ${rinfo.address}:${rinfo.port}`);

        let msg_str = msg.toString('utf8');
        let key = parseInt(msg_str.split(':')[0]);
        assert(key == begin);

        setTimeout(() => {
            send();
        }, 100);
    });

    send();
}

function run_udp_server(port) {
    const server = dgram.createSocket('udp4');

    server.on('error', (err) => {
        console.log(`server error:\n${err.stack}`);
        server.close();
    });

    server.on('message', (msg, rinfo) => {
        console.log(`server got: ${msg} from ${rinfo.address}:${rinfo.port}`);

        server.send(msg, rinfo.port, rinfo.address);
    });

    server.on('listening', () => {
        const address = server.address();
        console.log(`server listening ${address.address}:${address.port}`);
    });

    server.bind(port);
}

function run_one(client_port, server_port, begin) {
    run_udp_server(server_port);
    run_udp_client(client_port, begin);
}

run_one(99, 9998, 0);
run_one(99, 9998, 100000);

setInterval(() => {}, 1000);