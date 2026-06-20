# gathers-mirror

Standalone binary that snapshots every GatheRs card DB daily and serves them over plain HTTP. Lets you self-host a mirror so your `gathers`/`server` deployments (and anyone else's) stop hammering the upstream sources on every update.

## What it does

On startup, and then every `MIRROR_INTERVAL_HOURS` (default 24h), it refreshes five components into `MIRROR_DATA_DIR`:

| Stem | Source | Method |
|---|---|---|
| `AllPrintings.sqlite` | [mtgjson.com](https://mtgjson.com) | relays the upstream `.bz2` byte-for-byte, sha256-verified |
| `AllPricesToday.sqlite` | [mtgjson.com](https://mtgjson.com) | relays the upstream `.bz2` byte-for-byte, sha256-verified |
| `pokemon_prices.sqlite` | [poketrax/pokedata](https://github.com/poketrax/pokedata) | downloads raw sqlite, compresses itself |
| `riftbound.sqlite` | [Riftbound card gallery](https://riftbound.leagueoflegends.com/en-us/card-gallery/) | live scrape, compresses result |
| `pokemon.sqlite` | TCGPlayer / Serebii scrapers | live scrape, compresses result |

Each component is published as `{stem}.bz2` + `{stem}.bz2.sha256` in the data dir, and served as static files at `/`. One component failing (network hiccup, scraper target changing layout) doesn't block the others — it just logs and moves on to the next refresh cycle.

Writes are atomic: each file is built in a tempfile in the same directory, then renamed into place. Anyone downloading mid-refresh always gets either the complete old file or the complete new one, never a partial one.

## Running it

### Cargo

```bash
cargo run --bin mirror
```

### Docker

```bash
docker build -f Dockerfile.mirror -t gathers-mirror .
docker run -p 5235:5235 -v gathers-mirror-data:/home/app/.local/share/gathers/mirror gathers-mirror
```

### Docker Compose

Already wired into the repo's `docker-compose.yml` as the `gathers-mirror` service, alongside an inline `mirrors.toml` config that points `gathers-api` at it automatically.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `MIRROR_DATA_DIR` | `~/.local/share/gathers/mirror` | Where snapshots are written and served from |
| `MIRROR_PORT` | `5235` | HTTP port |
| `MIRROR_INTERVAL_HOURS` | `24` | Refresh cycle interval |

## Downloading manually

No auth, plain HTTP GET:

```bash
curl -O http://localhost:5235/AllPrintings.sqlite.bz2
curl -O http://localhost:5235/AllPrintings.sqlite.bz2.sha256
sha256sum AllPrintings.sqlite.bz2   # compare against the .sha256 file
```

Swap `AllPrintings.sqlite` for any other stem from the table above. Decompress with `bzip2 -d`.

## Pointing a GatheRs deployment at your mirror

On the client (the `gathers` CLI or `server` binary), create `~/.local/share/gathers/mirrors.toml` (override path via `GATHERS_MIRRORS_PATH`):

```toml
mirrors = [
  "https://your-mirror-host.example.com",
  "https://backup-mirror.example.com",
]
```

Ordered, highest priority first. Every retrieval system tries each mirror's `{stem}.bz2`/`.bz2.sha256` in order before falling back to the original upstream source unchanged. An empty/missing config is a no-op.

## Operating your own mirror

- **Leave its own `mirrors.toml` empty/unset.** The mirror's `run_update_cycle` always talks to true upstream directly — it doesn't call `try_mirrors` at all — but if you're chaining mirrors (e.g. a backup mirror pointed at a primary), make sure you don't create a cycle.
- **No auth, no TLS by default.** Put it behind a reverse proxy (Caddy/nginx/Traefik) if exposing it publicly — `MIRROR_PORT` binds `0.0.0.0` with no built-in auth.
- **Disk usage**: MTG cards (~650MB decompressed / ~170MB bz2) + prices (~25MB bz2) dominate. Budget a few hundred MB for the data dir.
- **First run is slow** (full scrapes + large downloads); subsequent runs only replace what changed upstream (MTG/prices skip re-downloading if the upstream sha256 sidecar is unchanged).
- **Restarts don't redo finished work.** Each of the five components tracks its own `{stem}.last_update` marker in the data dir, written only on success. On startup (and every cycle), a component refreshed within `MIRROR_INTERVAL_HOURS` is skipped outright; only stale or previously-failed/cancelled components are retried. So a restart mid-cycle re-attempts just what didn't finish, not everything.

## Why self-host one

Every GatheRs install hitting mtgjson.com, poketrax's GitHub, and Riftbound's live site on every `--download`/`/update` call adds up. A mirror means one polite daily pull per mirror operator instead of one per deployment. See the main [README](../README.md#db-mirror) for the full acknowledgements to the upstream projects this relies on.
