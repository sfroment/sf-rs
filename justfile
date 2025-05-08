# Available just commands
help:
	@echo "Available just commands:"
	@just --list

# Generate TypeScript types with typeshare
typeshare:
	#typeshare --lang typescript --output-dir ./sf-viz/generated-types ./src

# Run the example with the built server and a simple HTTP server
run-example: build-example
	@echo "Starting sf-server and a simple http server..."
	@trap 'kill 0' SIGINT; (./target/release/sf-server & python3 -m http.server 8081 & wait)


# Build both the wasm and server
build-example: build-wasm build-server

# Build the wasm package using wasm-pack
build-wasm:
	cd sf-wasm && wasm-pack build --target web

# Build the server using cargo
build-server:
	cd sf-server && cargo build --release

# Watch for changes in the wasm package and rebuild
watch-wasm:
	cd sf-wasm && cargo watch -i pkg -- wasm-pack build --target web --dev

# Build the Docker image for the server
docker-server:
	docker build -t sfromens/sf-server -f sf-server/Dockerfile .

# Run the Docker container for the server
run-server:
	docker run -p 8080:8080 sfromens/sf-server

# Run Docker Compose
docker-compose:
	docker compose -f docker/docker-compose.yml up -d

# Bring down Docker Compose
docker-compose-down:
	docker compose -f docker/docker-compose.yml down
