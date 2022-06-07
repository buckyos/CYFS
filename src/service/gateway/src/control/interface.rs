use super::{AssocServer, ControlServer, GATEWAY_CONTROL_SERVER};
//use cyfs_base::{BuckyError, BuckyErrorCode};
use tide::{Request, Response, StatusCode};


pub struct HttpControlInterface;

impl HttpControlInterface {
    pub fn new() -> HttpControlInterface {
        HttpControlInterface {}
    }

    pub fn init() {
        info!("will init http control interface...");

        let mut control_server = GATEWAY_CONTROL_SERVER.lock().unwrap();
        let app = control_server.get_server();
        //app.middleware(::tide::log::LogMiddleware::new());

        app.at("/register").post(|mut req: Request<()>| async move {
            //let mut server = server.clone();
            let resp = match req.body_string().await {
                Ok(v) => match ControlServer::register_server(&v) {
                    Ok(_) => {
                        let mut resp = Response::new(StatusCode::Ok);
                        let body = format!(r#"{{"code": "0", "msg": "Ok"}}"#,);
                        resp.set_body(body);

                        resp
                    }

                    Err(e) => {
                        let mut resp = Response::new(StatusCode::BadRequest);
                        let body = format!(r#"{{"code": "{:?}", "msg": "{}"}}"#, e.code(), e.msg());
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
            .post(|mut req: Request<()>| async move {
                //let mut server = server.clone();
                let resp = match req.body_string().await {
                    Ok(v) => match ControlServer::unregister_server(&v) {
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
            .get(|mut req: Request<()>| async move {
                let resp = match req.body_string().await {
                    Ok(v) => AssocServer::query(&v),
                    Err(e) => {
                        error!("read query request body error! err={}", e);

                        Response::new(StatusCode::BadRequest)
                    }
                };

                Ok(resp)
            });

        ControlServer::start_monitor();
    }
}
