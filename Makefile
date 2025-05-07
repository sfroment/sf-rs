.PHONY: help
help:
	@echo "Available make commands:"
	@cat Makefile | grep '^[a-z][^:]*:' | cut -d: -f1 | sort | sed 's/^/  /'

.PHONY: typeshare
typeshare:
    #typeshare --lang typescript --output-dir ./sf-viz/generated-types ./src

.PHONY: run-example
run-example: build-example build-server
	@echo "Starting sf-server and a simple http server..."
	( \
		trap 'kill 0' SIGINT; \
		./target/release/sf-server & \
		python3 -m http.server 8081 & \
		wait \
	)

.PHONY: build-example
build-example: build-wasm build-server

.PHONY: build-wasm
build-wasm:
	cd sf-wasm && wasm-pack build --target web

.PHONY: build-server
build-server:
	cd sf-server && cargo build --release

.PHONY: watch-wasm
watch-wasm:
	cd sf-wasm && cargo watch -- wasm-pack build --target web --dev

.PHONY: docker-server
docker-server:
	docker build -t sfromens/sf-server -f sf-server/Dockerfile .

.PHONY: run-server
run-server:
	docker run -p 8080:8080 sfromens/sf-server
