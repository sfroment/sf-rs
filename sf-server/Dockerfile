# Build stage
FROM rustlang/rust:nightly-slim as builder

WORKDIR /usr/src/sf-server

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY . .

RUN cd sf-server && cargo +nightly build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /usr/local/bin

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/sf-server/target/release/sf-server .

ENTRYPOINT ["sf-server"] 
