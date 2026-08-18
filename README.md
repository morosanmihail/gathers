# GatheRs

[Demo](https://demo.gathers.cards)

Collection of Rust crates and binaries to help one search for and manage Magic: the Gathering (TM) cards.
And Riftbound, apparently.
And Pokemon, it seems.
I really do like my modular code.

Contributions always welcome! 
I spend a lot of time designing the overall system to scale decently, but there are a lot of features I would like this to have, but don't have the time yet. One day, one day.

![Example of the UI](https://gathers.cards/images/webui2/collection1.png)

![Example of the CLI Tool](https://codeberg.org/morosanmihail/gathers/raw/branch/main/images/cli1.png)

![Prototype Riftbound support](https://codeberg.org/morosanmihail/gathers/raw/branch/riftbound/images/cli2.png)

# Codeberg vs Github

This repo is both on Codeberg and on Github.

I will read issues from Github, but Github is only a mirror.
Main development happens on Codeberg.
Support small tech!

# Installation and Setup

[Docker and Docker Compose](https://codeberg.org/morosanmihail/gathers/wiki/Docker-Setup)

# Info and Instructions and Ideas

[Can be found on the Wiki!](https://codeberg.org/morosanmihail/gathers/wiki/Home)

# DB Mirror

There's also a `mirror` binary/image. It snapshots all card DBs daily and serves them over HTTP, so a `gathers`/`server` deployment can pull from your own mirror instead of hitting the original sources every time.

Why: lessens load on the third parties who host this data for free. Big thanks to:
- [mtgjson.com](https://mtgjson.com) — MTG card and price data
- [poketrax/pokedata](https://github.com/poketrax/pokedata) — Pokémon price data
- [Riftbound's official card gallery](https://riftbound.leagueoflegends.com/en-us/card-gallery/) — Riftbound card data

Run it, then point a `gathers`/`server` deployment at it via `~/.local/share/gathers/mirrors.toml`:

```toml
mirrors = ["http://your-mirror-host:5235"]
```

Configured mirrors are tried first, in order, before falling back to the original source automatically.

A public mirror is already running at https://mirror.gathers.cards if you'd rather not host your own.

```toml
mirrors = ["http://mirror.gathers.cards"]
```

# Acknowledgements

Pokemon database scraper thanks to [pokedata](https://github.com/poketrax/pokedata).
GatheRs version is a Rust rewrite.

Riftbound database scraper thanks to [vikkumar2021](https://github.com/vikkumar2021/RiftboundCardDatabase). 
GatheRs version is a rust rewrite.

# Gallery

![Example of the UI, List View](https://gathers.cards/images/webui2/collection2.png)

![Example of the UI, Riftbound](https://gathers.cards/images/webui2/purchase1.png)
