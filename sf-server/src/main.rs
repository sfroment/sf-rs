mod args;
mod builder;
mod error;
mod logging;
mod peer_handler;
mod peer_id;
mod server;
mod state;
mod ws_handler;

use args::Args;
use axum::{Router, routing::get};
use builder::ServerBuilder;
use clap::Parser;
pub use error::Error;
use sf_metrics::InMemoryMetrics;
use state::AppState;
use std::sync::Arc;
use tracing::info;
use ws_handler::ws_handler;

#[tokio::main]
async fn main() {
    logging::setup_logging();
    let args = Args::parse();
    let metrics = InMemoryMetrics::new();
    let state = Arc::new(AppState::new(metrics));

    // TODO: Add the keychain package and make this coming from it
    info!("Building server on {}", args.host);
    let server = ServerBuilder::new(args.host)
        .mutate_router(|router| {
            let router: Router<()> = router.route("/ws", get(ws_handler)).with_state(state);
            router
        })
        .build();

    server.serve().await.unwrap();
}
