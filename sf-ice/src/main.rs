mod args;
mod builder;
mod error;
mod logging;
mod server;
mod ws_handler;
use std::convert::identity;

use args::Args;
use axum::{Extension, routing::get};
use builder::ServerBuilder;
use clap::Parser;
pub use error::Error;
use tracing::info;

#[tokio::main]
async fn main() {
    logging::setup_logging();
    let args = Args::parse();

    // TODO: Add the keychain package and make this coming from it
    let peer_id = "generated-peer-id".to_string();
    info!("Building server on {}", args.host);
    let server = ServerBuilder::new(args.host)
        .mutate_router(|router| router.route("/ws", get(ws_handler::ws_handler)))
        .mutate_router(|router| identity(router.layer(Extension(peer_id))))
        .build();

    server.serve().await.unwrap();
}
