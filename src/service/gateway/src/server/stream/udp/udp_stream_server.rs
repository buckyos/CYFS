use async_trait::async_trait;


use super::super::StreamServer;
use super::stream_bdt_datagram_listener::DatagramBdtListenerManager;
use super::stream_udp_listener::StreamUdpListenerManager;
use cyfs_base::BuckyError;

pub struct UdpStreamServer {
    proxy_pass: (String, u16),

    udp_listener_manager: StreamUdpListenerManager,
    bdt_listener_manager: DatagramBdtListenerManager,
}

impl UdpStreamServer {
    pub fn new() -> UdpStreamServer {
        UdpStreamServer {
            proxy_pass: (String::from(""), 0),
            udp_listener_manager: StreamUdpListenerManager::new(),
            bdt_listener_manager: DatagramBdtListenerManager::new(),
        }
    }

    fn load_listeners(&mut self, listener_list: &Vec<toml::Value>) -> Result<(), BuckyError> {
        for v in listener_list {
            if !v.is_table() {
                return BuckyError::error_with_log(format!(
                    "invalid stream block listener format, array or object was expected"
                ));
            }

            let listener_node = v.as_table().unwrap();
            let ret = self.load_listener(listener_node);
            if ret.is_err() {
                return ret;
            }
        }

        Ok(())
    }

    fn load_listener(&mut self, listener_node: &toml::value::Table) -> Result<(), BuckyError> {
        let mut listener_type = "udp";
        let v = listener_node.get("type");
        if v.is_some() {
            listener_type = v.unwrap().as_str().unwrap();
        }

        match listener_type {
            "udp" => {
                if let Err(e) = self.udp_listener_manager.load(listener_node) {
                    return Err(e);
                }
            }
            "bdt" => {
                if let Err(e) = self.bdt_listener_manager.load(listener_node) {
                    return Err(e);
                }
            }
            _ => {
                let msg = format!("unknown udp stream listener type: {:?}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        Ok(())
    }

    pub fn start_all(&self) {
        self.bdt_listener_manager.start();
        self.udp_listener_manager.start();
    }

    pub fn stop_all(&self) {
        self.bdt_listener_manager.stop();
        self.udp_listener_manager.stop();
    }
}

#[async_trait]
impl StreamServer for UdpStreamServer {
    fn load(&mut self, server_node: &toml::value::Table) -> Result<(), BuckyError> {
        for (k, v) in server_node {
            match k.as_str() {
                "block" => assert_eq!(v.as_str(), Some("server")),
                "listener" => {
                    if v.is_array() {
                        let _ret = self.load_listeners(v.as_array().unwrap());
                    } else if v.is_table() {
                        let _ret = self.load_listener(v.as_table().unwrap());
                    } else {
                        return BuckyError::error_with_log(format!(
                            "invalid stream block listener format, array or object was expected"
                        ));
                    }
                }
                "proxy_pass" => {
                    let proxy_pass = v.as_str().unwrap_or("");
                    let proxy_pass = ::base::VAR_MANAGER
                        .translate_addr_str(proxy_pass)?;

                    match cyfs_util::parse_address(&proxy_pass) {
                        Ok(ret) => self.proxy_pass = ret,
                        Err(e) => {
                            error!("invalid server block field: proxy_pass: {:?}, err={}", v, e);

                            return Err(e);
                        }
                    }
                }
                "id" | "protocol" => {}
                _ => {
                    warn!("unknown server block field: {}", k);
                }
            }
        }

        self.bdt_listener_manager.bind_proxy_pass(&self.proxy_pass);
        self.udp_listener_manager.bind_proxy_pass(&self.proxy_pass);

        Ok(())
    }

    fn start(&self) -> Result<(), BuckyError> {
        self.start_all();

        Ok(())
    }

    fn stop(&self) {
        self.stop_all();
    }
}
