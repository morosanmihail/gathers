GatheRs is a collection of Rust crates and binaries to help one search for and manage Magic: the Gathering (TM) cards, with experimental support for Riftbound and Pokémon cards.
Not to be confused with the really nice `gathers` crate, for clustering data.

# Crates

## retrieval

Find cards using search criteria.

Backends available:

- mtgjson SQLite DB (MTG)
- Scryfall.com (MTG)
- Riftbound SQLite DB (experimental)
- Pokémon SQLite DB (experimental)

## persistence

Persist a collection of cards to a database.

Also allows you to manipulate the collections.

## server

REST server to leverage the retrieval and persistence crates.

## webui2

A Svelte 5 / SvelteKit web ui to interact with the server.

Combined with the server, they allow you to use both the `retrieval` crate to search for cards, but also the `persistence` crate to store a database of the cards you own, or are interested in.

More information [here.](https://codeberg.org/morosanmihail/gathers/wiki/Server-and-WebUI)

## cli

Command line application to leverage the retrieval crates.

More information at [this page.](https://codeberg.org/morosanmihail/gathers/wiki/CLI)

# Setup

Docker instructions available [here.](https://codeberg.org/morosanmihail/gathers/wiki/Docker-Setup)
