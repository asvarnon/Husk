# Pin the builder to a bookworm base so its glibc matches the bookworm-slim
# runtime below. `rust:latest` tracks Debian trixie (glibc 2.38), which produces
# a binary the bookworm-slim runtime (glibc 2.36) cannot load. Keep these two in
# lockstep: if you bump the runtime to trixie, bump the builder to match.
FROM rust:bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/husk /usr/local/bin/husk
ENTRYPOINT ["/usr/local/bin/husk"]
