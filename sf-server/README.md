# sf-server

This server acts as a simple message forwarder for the WebSocket peers connected to it. It relays messages between peers based on the `PeerRequest::Forward` event defined in the `sf-protocol` crate.

## Overview

`sf-server` provides a central point for WebSocket clients (peers) to connect. When a peer sends a `PeerRequest::Forward` message, the server routes it:

- **To a specific peer:** If the `to_peer_id` field is set, the server forwards the message only to that peer.
- **Broadcast:** If the `to_peer_id` field is `None`, the server broadcasts the message to all other connected peers (excluding the sender).

When a new peer connects, the server automatically broadcasts a `PeerEvent::NewPeer` message (wrapped in `PeerRequest::Forward`) to all existing peers.

## Features

- **WebSocket Handling:** Manages WebSocket connections on the `/ws` endpoint.
- **Peer Management:** Uses a concurrent map (`DashMap`) to track connected peers.
- **Message Forwarding:** Routes messages based on the `PeerRequest` structure.
- **Peer Discovery:** Notifies existing peers when a new peer connects.
- **Metrics:** Integrates with `sf-metrics` for monitoring (e.g., peer count, messages forwarded).
- **Configurable Host:** Listens on a host address specified via command-line arguments.

## Usage

### Prerequisites

- Rust toolchain

### Building

```bash
cargo build --release
```

### Running

The server requires a host address to bind to.

```bash
# Example: Run the server listening on localhost:8080
./target/release/sf-server --host 127.0.0.1:8080
```

### Connecting Clients

WebSocket clients should connect to `ws://<server_host>/ws`. They can then send and receive `PeerRequest` messages serialized as JSON text frames, according to the definitions in the `sf-protocol` crate.

## Development

To build and run in development mode:

```bash
cargo run -- --host 127.0.0.1:8080
```

## License

This project is licensed under the terms of the project's root license. 
