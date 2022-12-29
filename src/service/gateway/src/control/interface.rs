use super::{AssocServer, ControlServer};
use crate::server::http::HttpServerManager;
use crate::server::stream::StreamServerManager;

use cyfs_base::*;
use once_cell::sync::OnceCell;
use tide::{Request, Response, StatusCode};

pub(crate) struct HttpControlInterface {
    control_server: ControlServer,
    server: OnceCell<tide::Server<ControlServer>>,
}

impl HttpControlInterface {
    pub fn new(
        stream_server_manager: StreamServerManager,
        http_server_manager: HttpServerManager,
    ) -> Self {
        Self {
            control_server: ControlServer::new(stream_server_manager, http_server_manager),
            server: OnceCell::new(),
        }
    }

    pub fn init(&self) {
        info!("will init http control interface...");

        let mut app = tide::Server::with_state(self.control_server.clone());

        //app.middleware(::tide::log::LogMiddleware::new());

        app.at("/register")
            .post(|mut req: Request<ControlServer>| async move {
                //let mut server = server.clone();
                let resp = match req.body_string().await {
                    Ok(v) => match req.state().register_server(&v) {
                        Ok(_) => {
                            let mut resp = Response::new(StatusCode::Ok);
                            let body = format!(r#"{{"code": "0", "msg": "Ok"}}"#,);
                            resp.set_body(body);

                            resp
                        }

                        Err(e) => {
                            let mut resp = Response::new(StatusCode::BadRequest);
                            let body =
                                format!(r#"{{"code": "{:?}", "msg": "{}"}}"#, e.code(), e.msg());
                            resp.set_body(body);

                            resp
                        }
                    },
                    Err(e) => {
                        error!("read register request body error! err={}", e);

                        Response::new(StatusCode::BadRequest)
                    }
                };

                Ok(resp)
            });

        app.at("/unregister")
            .post(|mut req: Request<ControlServer>| async move {
                //let mut server = server.clone();
                let resp = match req.body_string().await {
                    Ok(v) => match req.state().unregister_server(&v) {
                        Ok(_) => Response::new(StatusCode::Ok),
                        Err(e) => {
                            let mut resp = Response::new(StatusCode::BadRequest);
                            let body =
                                format!(r#"{{"code": "{:?}", "msg": "{}"}}"#, e.code(), e.msg());
                            resp.set_body(body);

                            resp
                        }
                    },
                    Err(e) => {
                        error!("read unregister request body error! err={}", e);

                        Response::new(StatusCode::BadRequest)
                    }
                };

                Ok(resp)
            });

        app.at("/peer_assoc")
            .get(|mut req: Request<ControlServer>| async move {
                let resp = match req.body_string().await {
                    Ok(v) => AssocServer::query(&v),
                    Err(e) => {
                        error!("read query request body error! err={}", e);

                        Response::new(StatusCode::BadRequest)
                    }
                };

                Ok(resp)
            });

        self.control_server.start_monitor();

        if let Err(_) = self.server.set(app) {
            unreachable!();
        }
    }

    pub async fn run(&self) -> BuckyResult<()> {
        let addr = cyfs_util::gateway::GATEWAY_CONTROL_URL;
        info!("gateway http control will run at {}", addr);

        // 注册完所有路由后，server就可以clone了
        let server = self.server.get().unwrap().clone();
        match server.listen(addr).await {
            Ok(_) => {
                info!("gateway control server finished!");
                Ok(())
            }
            Err(e) => {
                let msg = format!("gateway control server finished with error: {}", e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Failed, msg))
            }
        }
    }
}
