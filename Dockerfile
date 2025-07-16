FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/auto-dns /usr/local/bin/auto-dns
COPY config.toml.example /app/config.toml.example

# Create a non-root user
RUN useradd -r -s /bin/false auto-dns

USER auto-dns

ENTRYPOINT ["auto-dns"]
CMD ["--config", "/app/config.toml"]
