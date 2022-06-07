use std::{net::{Shutdown, IpAddr, Ipv4Addr, SocketAddr}, str::FromStr};
use async_std::{
    net::TcpStream, 
    io::prelude::*
};
use cyfs_bdt;

#[async_std::main]
async fn main() {
    let matches = cyfs_bdt::debug::debug_command_line()
        .get_matches();
    
    let remote_str = matches.value_of("host").unwrap_or("127.0.0.1");
    let server_port_str = matches.value_of("port").unwrap_or("12345");
    let server_port =std::str::FromStr::from_str(server_port_str).unwrap();
    
    let cmd_line = std::env::args().collect::<Vec<String>>().join(" ");

    let stub_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from_str(remote_str).unwrap()), server_port);
    
    let mut res_last_line = String::new();
    let mut exit_code = 1;
    let cmd = get_cmd();
    if let Ok(stream) = TcpStream::connect(stub_addr).await {
        let mut stream = stream;
        if let Ok(_) = stream.write_all(cmd_line.as_ref()).await {
            let _ = stream.shutdown(Shutdown::Write);
            let mut reader = async_std::io::BufReader::new(stream);
            loop {
                let mut buf = String::new();
                let line = reader.read_line(&mut buf).await;
                match line {
                    Ok(len) => {
                        if len == 0 {
                            exit_code = process_exit_code(cmd, res_last_line);
                            break ;
                        }
                        res_last_line = buf.clone();
                        print!("{}", buf);
                    },
                    Err(e) => {
                        println!("read err: {}", e);
                        break ;
                    }
                }
            }
        } else {
            println!("debuger deamon no return");
        }
    } else {
        println!("debuger deamon not exists (port:{})", server_port);
    }

    std::process::exit(exit_code);
}

fn get_cmd() -> String {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 2 {
        args[1].clone()
    } else {
        String::new()
    }
}

fn process_exit_code(cmd: String, res_last_line: String) -> i32 {
    match cmd.as_str() {
        "sn_conn_status" | "nc" => {
            if res_last_line.starts_with("Ok:") {
                0
            } else {
                1
            }
        },
        _ => 0
    }
}