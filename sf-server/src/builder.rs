use axum::Router;
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{
        DefaultOnBodyChunk, DefaultOnEos, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse,
        TraceLayer,
    },
};
use tracing::Level;

use crate::server::Server;

pub struct ServerBuilder<S = ()> {
    addr: SocketAddr,
    router: Router<S>,
}

impl ServerBuilder<()> {
    pub fn build(self) -> Server {
        Server::new(
            self.addr,
            self.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
    }
}

impl<S> ServerBuilder<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new(addr: impl Into<SocketAddr>) -> Self {
        Self {
            addr: addr.into(),
            router: Router::new(),
        }
        .default_middleware()
    }

    pub fn mutate_router<R, T>(self, alter: R) -> ServerBuilder<T>
    where
        T: Clone + Send + Sync + 'static,
        R: FnOnce(Router<S>) -> Router<T>,
    {
        let router = alter(self.router);
        ServerBuilder {
            addr: self.addr,
            router,
        }
    }

    fn default_middleware(self) -> Self {
        self.logging_middleware().allow_any_cors()
    }

    fn logging_middleware(self) -> Self {
        self.mutate_router(|router| {
            let default_response = DefaultOnResponse::new().level(Level::DEBUG);
            let default_request = DefaultOnRequest::new().level(Level::DEBUG);
            let default_failure = DefaultOnFailure::new().level(Level::DEBUG);
            let default_eos = DefaultOnEos::new().level(Level::DEBUG);
            let default_body_chunk = DefaultOnBodyChunk::new();

            let trace_layer = TraceLayer::new_for_http()
                .on_response(default_response)
                .on_request(default_request)
                .on_failure(default_failure)
                .on_eos(default_eos)
                .on_body_chunk(default_body_chunk);

            router.layer(trace_layer)
        })
    }

    fn allow_any_cors(self) -> Self {
        self.mutate_router(|router| {
            let cors_layer = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            router.layer(cors_layer)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{response::IntoResponse, routing::get};
    use std::net::SocketAddr;

    async fn test_handler() -> impl IntoResponse {
        "test"
    }

    #[test]
    fn test_new_builder() {
        let addr = "127.0.0.1:3000".parse::<SocketAddr>().unwrap();
        let builder: ServerBuilder<()> = ServerBuilder::new(addr);

        assert_eq!(builder.addr, addr);
    }

    #[test]
    fn test_mutate_router() {
        let addr = "127.0.0.1:3000".parse::<SocketAddr>().unwrap();
        let builder: ServerBuilder<()> = ServerBuilder::new(addr);

        let _modified_builder =
            builder.mutate_router(|router| router.route("/test", get(test_handler)));
    }

    #[test]
    fn test_default_middleware() {
        let addr = "127.0.0.1:3000".parse::<SocketAddr>().unwrap();
        let builder: ServerBuilder<()> = ServerBuilder::new(addr);

        let _builder_with_middleware = builder.default_middleware();
    }

    #[test]
    fn test_allow_any_cors() {
        let addr = "127.0.0.1:3000".parse::<SocketAddr>().unwrap();
        let builder: ServerBuilder<()> = ServerBuilder::new(addr);

        let _builder_with_cors = builder.allow_any_cors();
    }

    #[test]
    fn test_build() {
        let addr = "127.0.0.1:3000".parse::<SocketAddr>().unwrap();
        let builder: ServerBuilder<()> = ServerBuilder::new(addr);

        let server = builder.build();

        assert_eq!(server.addr, addr);
    }
}
