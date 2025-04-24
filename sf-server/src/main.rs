#![feature(coverage_attribute)]
#![deny(warnings)]

mod args;
mod builder;
mod error;
mod extract_peer_id;
mod peer_handler;
mod peer_id;
mod server;
mod socket_metadata;
mod state;
mod ws;

use args::Args;
use axum::{Router, routing::get};
use builder::ServerBuilder;
use clap::Parser;
pub use error::Error;
use sf_metrics::InMemoryMetrics;
use state::AppState;
use std::sync::Arc;
use tracing::info;
use ws::ws_handler;

async fn run(args: Args) -> Result<(), Error> {
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

    server.serve().await
}

#[tokio::main]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn main() {
    // logging::setup_logging();
    let args = Args::parse();
    if let Err(e) = run(args).await {
        eprintln!("Error running server: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_main_function() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();
        drop(listener);

        let args = Args { host: server_addr };

        let server_task = tokio::spawn(async move {
            run(args).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut connected = false;
        let websocket_url = format!("ws://{server_addr}/ws?peer_id=test_main");

        for i in 0..5 {
            tokio::time::sleep(Duration::from_millis(100 * (i + 1))).await;

            match tokio_tungstenite::connect_async(&websocket_url).await {
                Ok((mut ws_stream, response)) => {
                    assert_eq!(response.status(), 101);
                    ws_stream.close(None).await.unwrap();
                    connected = true;
                    break;
                }
                Err(e) => {
                    if i == 4 {
                        println!("Failed to connect after 5 attempts: {e}");
                    }
                }
            }
        }

        server_task.abort();

        assert!(connected, "Failed to connect to WebSocket server");
    }
}
