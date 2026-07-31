# syntax=docker/dockerfile:1
FROM rust:1.92-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential libsqlite3-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --locked --bin server \
    && cp target/release/server /server

FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash app
USER app
WORKDIR /home/app

RUN mkdir -p /home/app/.local/share/gathers/DB

COPY --from=builder /server ./server

EXPOSE 5234

ENV STORAGE_DB_PATH=/home/app/.local/share/gathers/DB/storage.db
ENV MTG_DB_PATH=/home/app/.local/share/gathers/DB/AllPrintings.db
ENV RIFTBOUND_DB_PATH=/home/app/.local/share/gathers/DB/riftbound.db
ENV POKEMON_DB_PATH=/home/app/.local/share/gathers/DB/pokemon.db

ENTRYPOINT ["/home/app/server"]
