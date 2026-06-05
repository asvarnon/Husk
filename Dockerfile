FROM rust:latest AS builder
WORKDIR /workspace
COPY . .
RUN cargo build --release -p homelab-discord

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /workspace/target/release/homelab-discord /usr/local/bin/homelab-discord
ENTRYPOINT ["/usr/local/bin/homelab-discord"]
