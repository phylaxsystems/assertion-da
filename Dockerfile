# syntax=docker/dockerfile:1.2
FROM rustlang/rust:nightly AS chef

RUN cargo install cargo-chef

WORKDIR /assertion-da

FROM chef AS planner

COPY crates ./crates
COPY Cargo.* .

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /assertion-da/recipe.json recipe.json

# Build dependencies only, these remained cached
RUN cargo chef cook --release --recipe-path recipe.json

# Optional build flags
ARG BUILD_FLAGS=""
COPY crates ./crates
COPY Cargo.* .
RUN cargo build --release --locked $BUILD_FLAGS

FROM ubuntu:24.04
WORKDIR /usr/local/bin

COPY --from=builder /assertion-da/target/release/assertion-da /usr/local/bin/assertion-da

EXPOSE 5001

ENTRYPOINT ["/usr/local/bin/assertion-da"]