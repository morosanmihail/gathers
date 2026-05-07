FROM rust:1.92 as builder

WORKDIR /app

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./gathers/Cargo.toml ./gathers/Cargo.toml
COPY ./server/Cargo.toml ./server/Cargo.toml
COPY ./persistence/Cargo.toml ./persistence/Cargo.toml
COPY ./retrieval/Cargo.toml ./retrieval/Cargo.toml
COPY ./models/Cargo.toml ./models/Cargo.toml
COPY ./benches/Cargo.toml ./benches/Cargo.toml

COPY ./gathers/src ./gathers/src
COPY ./server/src ./server/src
COPY ./persistence/src ./persistence/src
COPY ./retrieval/src ./retrieval/src
COPY ./models/src ./models/src
COPY ./benches/src ./benches/src
COPY ./persistence/migrations ./persistence/migrations

RUN cargo build --release --bin server

FROM ubuntu:24.04

RUN apt-get update && apt-get install -y ca-certificates libsqlite3-0

RUN useradd --create-home --shell /bin/bash app
USER app
WORKDIR /home/app

RUN mkdir -p /home/app/.local/share/gathers/DB

COPY --from=builder /app/target/release/server .

EXPOSE 5234

ENV STORAGE_DB_PATH=/home/app/.local/share/gathers/DB/storage.db
ENV MTG_DB_PATH=/home/app/.local/share/gathers/DB/AllPrintings.db
ENV RIFTBOUND_DB_PATH=/home/app/.local/share/gathers/DB/riftbound.db
ENV POKEMON_DB_PATH=/home/app/.local/share/gathers/DB/pokemon.db

ENTRYPOINT ["/home/app/server"]
