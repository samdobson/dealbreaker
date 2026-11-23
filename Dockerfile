# syntax=docker/dockerfile:1

FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN RUSTFLAGS="-C target-feature=+crt-static -C strip=symbols" \
    cargo build --release --locked -p dealbreaker-cli

FROM scratch
LABEL org.opencontainers.image.title="Dealbreaker" \
      org.opencontainers.image.description="A fast patience (solitaire) solver written in Rust, where every game is data." \
      org.opencontainers.image.url="https://samdobson.github.io/dealbreaker/" \
      org.opencontainers.image.source="https://github.com/samdobson/dealbreaker" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"
COPY --from=builder /src/target/release/dealbreaker /dealbreaker
USER 65532:65532
ENTRYPOINT ["/dealbreaker"]
