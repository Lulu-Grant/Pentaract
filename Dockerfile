############################################################################################
####  SERVER
############################################################################################

# Using the `rust-musl-builder` as base image, instead of 
# the official Rust toolchain
FROM clux/muslrust:stable AS chef
USER root
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY ./pentaract .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY ./pentaract .
RUN cargo build --release
RUN cp "$(find /app/target -path '*/release/pentaract' -type f | head -n 1)" /pentaract-bin

############################################################################################
####  UI
############################################################################################

FROM node:21-slim AS ui
WORKDIR /app
COPY ./ui .
RUN npm install -g pnpm@8.15.9
RUN pnpm install --frozen-lockfile
ENV VITE_API_BASE /api
RUN pnpm run build

############################################################################################
####  RUNNING
############################################################################################

# We do not need the Rust toolchain to run the binary!
FROM scratch AS runtime
COPY --from=builder /pentaract-bin /pentaract
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=ui /app/dist /ui
ENTRYPOINT ["/pentaract"]
