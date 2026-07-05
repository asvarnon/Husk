# Both stages MUST be Debian trixie (glibc 2.38+). The ONNX Runtime prebuilt
# pulled in via fastembed -> ort -> ort-sys is compiled against glibc 2.38 and
# references its C23 strtol symbols (__isoc23_strtol/strtoll/strtoull), so:
#   - the builder needs glibc 2.38 or the final link fails with "undefined
#     symbol: __isoc23_strtoll",
#   - the runtime needs glibc 2.38 or the binary won't load ("GLIBC_2.38 not
#     found").
# Keep the two suites identical. Do not drop either back to bookworm (2.36)
# unless the ORT prebuilt's glibc floor drops with it.
FROM rust:trixie AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/husk /usr/local/bin/husk
ENTRYPOINT ["/usr/local/bin/husk"]
