use super::{StreamServer, TcpStreamServer, UdpStreamServer};
use cyfs_base::{BuckyError, BuckyErrorCode};

pub struct StreamServerManager {
    server_list: Vec<(String, Box<dyn StreamServer>)>,
}

impl StreamServerManager {
    pub fn new() -> StreamServerManager {
        StreamServerManager {
            server_list: Vec::new(),
        }
    }

    pub fn load(&mut self, stream_node: &Vec<toml::Value>) -> Result<(), BuckyError> {
        assert!(self.server_list.len() == 0);

        for v in stream_node {
            let node = v.as_table();
            if node.is_none() {
                continue;
            }

            let node = node.unwrap();
            let block = node.get("block");
            if block.is_none() {
                continue;
            }

            let block = block.unwrap().as_str();
            if block.is_none() {
                continue;
            }

            match block {
                Some("server") => {
                    // FIXME 加载出错是否要继续？
                    let _r = self.load_server(node);
                }
                _ => {
                    warn!("unknown stream.block type: {:?}", block);
                }
            }
        }

        Ok(())
    }

    pub fn load_server(&mut self, server_node: &toml::value::Table) -> Result<(), BuckyError> {
        let id = match server_node.get("id") {
            Some(toml::Value::String(v)) => v,
            _ => {
                let msg = format!("invalid server node id field! node={:?}", server_node);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, &msg));
            }
        };

        // 目前只tcp类型
        match server_node.get("protocol") {
            Some(toml::Value::String(v)) if v == "tcp" => {
                let mut server = TcpStreamServer::new();
                if let Err(e) = server.load(server_node) {
                    error!("load stream tcp server node error: id={}, err={}", id, e);

                    return Err(BuckyError::from(e));
                }

                self.server_list.push((id.to_owned(), Box::new(server)));

                Ok(())
            }
            Some(toml::Value::String(v)) if v == "udp" => {
                let mut server = UdpStreamServer::new();
                if let Err(e) = server.load(server_node) {
                    error!("load stream udp server node error: id={}, err={}", id, e);

                    return Err(BuckyError::from(e));
                }

                self.server_list.push((id.to_owned(), Box::new(server)));

                Ok(())
            }
            _ => {
                let msg = format!("invalid stream protocol! node={:?}", server_node);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidParam, &msg))
            }
        }
    }

    pub fn start(&self) {
        for (_id, server) in &self.server_list {
            let _r = server.start();
            // let _r = server.start().await;
        }
    }

    // 停止并移除指定的server block
    pub fn remove_server(&mut self, id: &str) -> Result<(), BuckyError> {
        let pos = match self.server_list.iter().position(|v| v.0 == id) {
            Some(pos) => pos,
            None => {
                let msg = format!("stream server not found! id={}", id);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::NotFound, &msg));
            }
        };

        info!("will remove stream server: {}", id);
        let server = self.server_list.remove(pos);
        server.1.stop();

        Ok(())
    }
}
