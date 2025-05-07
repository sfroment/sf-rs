<div align="center">
  <h1><code>sf-X</code></h1>

  <strong>Experiment around webRTC/Wasm</strong>
</div>

## Project Structure

- `sf-wasm/` - WebAssembly module that handles WebRTC peer connections and data channels
- `sf-server/` - Signaling server that helps establish WebRTC connections between peers
- `sf-webrtc/` - WebRTC bindings and utilities
- `sf-protocol/` - Communication protocol definitions
- `sf-peer-id/` - Peer identification and management
- `sf-metrics/` - Metrics collection and monitoring
- `sf-logging/` - Logging utilities

## Prerequisites

- Rust and Cargo
- wasm-pack
- Python 3 (for running the example server)
- Docker (optional, for running the server in a container)

## Building and Running

### Using Make Commands

The project provides several make commands to help with development:

```bash
# Build the WebAssembly module
make build-wasm

# Build the signaling server
make build-server

# Build both the example and server
make build-example

# Run the example (starts both the server and a simple HTTP server)
make run-example

# Watch for changes in the WebAssembly code and rebuild automatically
make watch-wasm

# Build the server Docker image
make docker-server

# Run the server using Docker
make run-server
```

### Development Workflow

1. Start the signaling server:
   ```bash
   make run-server
   ```

2. In a separate terminal, start the development server:
   ```bash
   make run-example
   ```

3. Open your browser to `http://localhost:8081` to see the example in action.

## Docker Support

The project includes Docker support for running the signaling server:

```bash
# Build the Docker image
make docker-server

# Run the server container
make run-server
```

## License

This project is licensed under the terms of the license included in the repository.
