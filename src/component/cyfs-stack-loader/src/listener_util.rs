use crate::{STACK_MANAGER, VAR_MANAGER};
use cyfs_base::{BuckyError, BuckyErrorCode};

use std::net::SocketAddr;

pub struct ListenerUtil;

impl ListenerUtil {
    pub fn load_tcp_listener(
        server_node: &toml::value::Table,
    ) -> Result<Vec<SocketAddr>, BuckyError> {
        Self::load_native_listener("tcp", server_node)
    }

    pub fn load_udp_listener(
        server_node: &toml::value::Table,
    ) -> Result<Vec<SocketAddr>, BuckyError> {
        Self::load_native_listener("udp", server_node)
    }

    /*
    加载以下内容到socket_addr列表
    {
        type: "tcp|udp",
        listen: "127.0.0.1:80",
    }
    */
    pub fn load_native_listener(
        listener_type: &str,
        server_node: &toml::value::Table,
    ) -> Result<Vec<SocketAddr>, BuckyError> {
        let mut addr_list: Vec<SocketAddr> = Vec::new();

        for (k, v) in server_node {
            match k.as_str() {
                "type" => assert_eq!(v.as_str().unwrap_or(listener_type), listener_type),
                "listen" => {
                    let parts: Vec<&str> = v.as_str().unwrap_or("").split(":").collect();
                    if parts.len() != 2 {
                        let msg = format!(
                            "invalid listen address and port! format=[address]:[port], got={:?}",
                            v
                        );
                        error!("{}", msg);

                        return Err(BuckyError::from(msg));
                    }

                    // 解析地址列表
                    addr_list = VAR_MANAGER.replace_socket_addr_and_parse(parts[0])?;

                    // 解析端口
                    let port = parts[1].parse::<u16>().map_err(|e| {
                        let msg = format!("parse listen port error! value={}, {}", parts[1], e);
                        error!("{}", e);

                        BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                    })?;

                    // 绑定端口
                    addr_list.iter_mut().for_each(|addr| addr.set_port(port));
                }
                _ => {
                    // error!("unknown tcp server field: {}", k.as_str());
                }
            }
        }

        if addr_list.is_empty() {
            let msg = format!("invalid tcp listener block, listen field not found!");
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        Ok(addr_list)
    }

    /*
    {
        stack: "bdt_public",
        vport: 80,
    }
    */
    pub fn load_bdt_listener(
        server_node: &toml::value::Table,
    ) -> Result<(String, u16), BuckyError> {
        let mut stack: Option<String> = None;
        let mut vport: Option<u16> = None;

        for (k, v) in server_node {
            match k.as_str() {
                //"type" => assert_eq!(v.as_str().unwrap_or("bdt"), "bdt"),
                "stack" => {
                    let stack_str = v.as_str().unwrap_or("");
                    if stack_str.is_empty() {
                        return BuckyError::error_with_log(format!(
                            "bdt listener stack field is empty! v={:?}",
                            v
                        ));
                    }
                    stack = Some(v.as_str().unwrap().to_owned());
                }
                "vport" => match cyfs_util::parse_port_from_toml_value(v) {
                    Ok(port) => vport = Some(port),
                    Err(e) => {
                        return Err(e);
                    }
                },
                _ => {
                    // error!("unknown tcp server field: {}", k.as_str());
                }
            }
        }

        if stack.is_none() || vport.is_none() {
            return BuckyError::error_with_log(format!(
                "invalid bdt server block, stack or vport not specified! v={:?}",
                server_node
            ));
        }

        // 检查bdt stack是否存在
        let stack = stack.unwrap();
        if !STACK_MANAGER.exists(stack.as_str()) {
            return BuckyError::error_with_log(format!(
                "bdt server stack not found! stack={}",
                stack
            ));
        }

        Ok((stack, vport.unwrap()))
    }
}
