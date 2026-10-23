# syntax=docker/dockerfile:1
FROM rust:1.86-slim-bookworm@sha256:57d415bbd61ce11e2d5f73de068103c7bd9f3188dc132c97cef4a8f62989e944 AS base

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown

# Install Stellar CLI (same flags as CI)
RUN cargo install --locked stellar-cli --features opt

WORKDIR /app
COPY . .

CMD ["bash"]
