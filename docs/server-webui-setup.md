![Example of the UI](https://codeberg.org/morosanmihail/hometg/raw/branch/main/images/ui20230628.jpg)

# How To Start

The following command spins up both the React webUI, as well as the backend axum server.

```bash
npm start
```

You can start them individually by doing the following for the webui:

```bash
npm run start-webui
```

Or the following for the backend server:

```bash
cargo run --bin server -- --system sql --port 5234
```

The `--system` flag may be specified multiple times to enable multiple retrieval backends simultaneously. Supported values: `scryfall`, `sql`, `riftbound-sql`, `pokemon-sql`.

```bash
# Enable both MTG and Riftbound
cargo run --bin server -- --system sql --system riftbound-sql --port 5234
```

You can then access the webui at `http://localhost:3000`.

## Config File

On first run with `--system` and `--port`, the server writes a config file to `~/.local/share/gathers/server.toml`. On subsequent runs, this config is loaded automatically and `--system`/`--port` are not required.

## Environment Variables

Environment variables override the paths set in the config file:

| Variable | Default | Description |
|---|---|---|
| `MTG_DB_PATH` | `~/.local/share/gathers/DB/AllPrintings.db` | MTG SQLite database (`sql` system) |
| `MTG_PRICES_PATH` | `~/.local/share/gathers/DB/AllPricesToday.json` | MTG price data (`sql` system) |
| `RIFTBOUND_DB_PATH` | `~/.local/share/gathers/DB/riftbound.db` | Riftbound SQLite database (`riftbound-sql` system) |
| `POKEMON_DB_PATH` | `~/.local/share/gathers/DB/pokemon.db` | Pokémon SQLite database (`pokemon-sql` system) |
| `STORAGE_DB_PATH` | `~/.local/share/gathers/DB/storage.db` | User collection database |

## Config File Options

The `server.toml` config supports these keys beyond `system` and `port`:

| Key | Default | Description |
|---|---|---|
| `pricing_enabled` | `true` | Show market prices, purchase price inputs, and purchase history in the UI |
| `mtg_db_path` | see env var default | Path to `AllPrintings.db` |
| `mtg_prices_path` | see env var default | Path to `AllPricesToday.json` |
| `riftbound_db_path` | see env var default | Path to `riftbound.db` |
| `pokemon_db_path` | see env var default | Path to `pokemon.db` |
| `storage_db_path` | see env var default | Path to `storage.db` |

All config options can also be changed at runtime via the Settings page in the web UI (`/settings`). Changes to `pricing_enabled` take effect immediately; other changes (paths, port, systems) require a server restart.

## Retrieval Database

The `sql` system requires the MTG database from www.mtgjson.com. You can trigger a background download via the `/mtg/update` endpoint:

```bash
curl http://localhost:5234/mtg/update -H "Accept: application/json"
```

Similarly:
- Riftbound database: `/riftbound/update`
- Pokémon database: `/pokemon/update`
- MTG prices: `/mtg/prices/update`
