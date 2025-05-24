mod bindings;
mod callback;
mod client;
mod log;
mod logging;
mod peer;
mod peer_manager;
mod websocket;
mod proto {
    include!("proto/keep_alive.rs");
}

use anyhow::Context;
use gloo_timers::future::IntervalStream;

use gloo_net::websocket::Message;
use moq_transfork::web_transport;
use proto::{KeepAliveRequest, keep_alive_client::KeepAliveClient};
use std::{cell::RefCell, rc::Rc, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic_web_wasm_client::Client as WasmClient;
use tracing::{info, trace};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

pub use client::*;

#[wasm_bindgen]
pub async fn test_grpc() -> Result<(), JsError> {
    let mut client = KeepAliveClient::new(WasmClient::new("https://localhost:4433".to_string()));

    let (tx, rx) = mpsc::channel::<KeepAliveRequest>(10); // Buffer of 10, adjust as needed
    let mpsc_receiver_for_grpc = ReceiverStream::new(rx);

    // --- Task to Periodically Send Requests ---
    spawn_local(async move {
        // tx is moved into this task
        tracing::info!("[WASM Finite Sender] Sending priming message...");
        if tx.send(KeepAliveRequest {}).await.is_err() {
            tracing::error!("[WASM Finite Sender] Prime send failed.");
            return; // tx is dropped here
        }
        tracing::info!("[WASM Finite Sender] Sending second message...");
        if tx.send(KeepAliveRequest {}).await.is_err() {
            tracing::error!("[WASM Finite Sender] Second send failed.");
            return; // tx is dropped here
        }
        tracing::info!(
            "[WASM Finite Sender] All messages sent. Sender will be dropped, ending the stream."
        );
        // tx is dropped when this task scope ends
    });
    // --- End Finite Sender Task ---

    // Give a moment for the sender task to potentially run and send, and drop.
    gloo_timers::future::TimeoutFuture::new(100).await; // 100ms

    tracing::info!("[WASM Main] Calling client.keep_alive with finite MPSC stream. Awaiting...");
    match client.keep_alive(mpsc_receiver_for_grpc).await {
        Ok(response) => {
            tracing::info!(
                "[WASM Main] Finite MPSC: keep_alive resolved OK: {:?}",
                response
            );
            // ... try to process response stream ...
        }
        Err(e) => {
            tracing::error!(
                "[WASM Main] Finite MPSC: keep_alive resolved with ERROR: {:?}",
                e
            );
        }
    }
    // Dummy Ok return for the example
    Ok(())
}

#[wasm_bindgen]
pub async fn test_moq() -> Result<(), JsError> {
    trace!("test");
    let client = web_transport::ClientBuilder::new()
        .with_congestion_control(web_transport::CongestionControl::LowLatency);

    let fingerprint =
        hex::decode("a475c23ad4081aff0babecabf0d7201c11f6e6c2d1757479d53182ba8f16eb45").unwrap();
    let client = client.with_server_certificate_hashes(vec![fingerprint])?;

    let url = url::Url::parse("https://localhost:4433").unwrap();
    let session = client.connect(&url).await?;
    let session = moq_transfork::Session::connect(session).await?;
    let track = moq_transfork::Track::new("test");
    let mut consumer = session.subscribe(track);
    info!("path: {:?}", consumer.path);
    info!("Waiting for group...");
    match consumer.next_group().await {
        Ok(Some(mut group)) => {
            let frame = group.read_frame().await?;
            tracing::info!("Received frame: {:?}", frame);
        }
        Ok(None) => {
            tracing::error!("No group received");
        }
        Err(e) => {
            tracing::error!("Error receiving group: {:?}", e);
        }
    }
    info!("path: {:?}", consumer.path);
    info!("Done");

    Ok(())
}
