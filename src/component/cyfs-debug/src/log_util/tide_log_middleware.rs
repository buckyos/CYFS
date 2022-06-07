use std::future::Future;
use std::pin::Pin;
use tide::{Middleware, Next, Request, Response};

/// Log all incoming requests and responses.
///
/// This middleware is enabled by default in Tide.
///
/// # Examples
///
/// ```
/// let mut app = tide::Server::new();
/// app.middleware(tide::log::LogMiddleware::new());
/// ```
#[derive(Debug, Default, Clone)]
pub struct LogMiddleware {
    _priv: (),
}

impl LogMiddleware {
    /// Create a new instance of `LogMiddleware`.
    #[must_use]
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Log a request and a response.
    async fn log<'a, State: Send + Sync + Clone + 'static>(
        &'a self,
        ctx: Request<State>,
        next: Next<'a, State>,
    ) -> tide::Result {
        let path = ctx.url().path().to_owned();
        let method = ctx.method().to_string();
        log::info!("<-- Request received {} {}", method, path);
        let start = std::time::Instant::now();
        let res = next.run(ctx).await;

        let status = res.status();
        if status.is_server_error() {
            log::error!(
                "--> Response sent {} {} {} {}",
                method,
                path,
                status,
                format!("{:?}", start.elapsed())
            );
        } else if status.is_client_error() {
            log::warn!(
                "--> Response sent {} {} {} {}",
                method,
                path,
                status,
                format!("{:?}", start.elapsed())
            );
        } else {
            log::debug!(
                "--> Response sent {} {} {} {}",
                method,
                path,
                status,
                format!("{:?}", start.elapsed())
            );
        }
        Ok(res)
    }
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl<State: Send + Sync + Clone + 'static> Middleware<State> for LogMiddleware {
    fn handle<'a, 'b, 't>(
        &'a self,
        ctx: Request<State>,
        next: Next<'b, State>,
    ) -> BoxFuture<'t, tide::Result<Response>>
    where
        'a: 't,
        'b: 't,
        Self: 't,
    {
        Box::pin(async move { self.log(ctx, next).await })
    }
}
