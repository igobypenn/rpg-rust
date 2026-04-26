FROM rust:1.82-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release -p rpg-mcp

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/rpg-mcp /usr/local/bin/rpg-mcp
ENTRYPOINT ["rpg-mcp"]
