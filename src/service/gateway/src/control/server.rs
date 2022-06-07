use lazy_static::lazy_static;
use std::sync::Mutex;
use tide::Server;
use cyfs_util::gateway::GATEWAY_CONTROL_URL;

pub struct GatewayControlServer {
    server: Server<()>,
}

impl GatewayControlServer {
    pub fn new() -> Self {
        Self {
            server: tide::new(),
        }
    }

    pub async fn run() {
        // 注册完所有路由后，server就可以clone了
        let server = GATEWAY_CONTROL_SERVER.lock().unwrap().get_server().clone();
        let addr = GATEWAY_CONTROL_URL;
        info!("gateway http control will run at {}", addr);

        match server.listen(addr).await {
            Ok(_) => {
                info!("gateway control server finished!");
            }
            Err(e) => {
                error!("gateway control server finished with error: {}", e);
            }
        }
    }

    pub fn get_server(&mut self) -> &mut Server<()> {
        &mut self.server
    }
}

lazy_static! {
    pub(crate) static ref GATEWAY_CONTROL_SERVER: Mutex<GatewayControlServer> =
        Mutex::new(GatewayControlServer::new());
}


