use std::sync::Arc;


#[derive(Clone)]
pub struct HttpServer {
    server: Arc<::tide::Server<()>>,
}

impl HttpServer {
    pub fn new_server() -> ::tide::Server<()> {
        use http_types::headers::HeaderValue;
        use tide::security::{CorsMiddleware, Origin};

        let mut server = ::tide::new();

        let cors = CorsMiddleware::new()
            .allow_methods("GET, POST, OPTIONS".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
        server.with(cors);

        server
    }

    pub fn new(server: tide::Server<()>) -> Self {
        Self {
            server: Arc::new(server),
        }
    }

    pub fn server(&self) -> &Arc<::tide::Server<()>> {
        &self.server
    }
    
    pub async fn respond(
        &self,
        req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        self.server.respond(req).await
    }
}
