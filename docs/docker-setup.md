To run the server using Docker, you can use the provided Dockerfile or docker-compose.yml.

## Docker Compose Configuration

```yaml
services:
  gathers-api:
    image: ghcr.io/morosanmihail/gathers-api:latest
    ports:
      - "5234:5234"
    volumes:
      - gathers-data:/home/app/.local/share/gathers/:rw
    environment:
      # Comma-separated list of systems to enable.
      # Valid values: scryfall, sql, riftbound-sql, pokemon-sql
      # Add "sql" for offline MTG (downloads AllPrintings.db on first start).
      # Add "pokemon-sql" for Pokémon TCG support.
      - GATHERS_SYSTEMS=riftbound-sql,scryfall
      - STORAGE_DB_PATH=/home/app/.local/share/gathers/DB/storage.db
      - MTG_DB_PATH=/home/app/.local/share/gathers/DB/AllPrintings.db
      - RIFTBOUND_DB_PATH=/home/app/.local/share/gathers/DB/riftbound.db
      - POKEMON_DB_PATH=/home/app/.local/share/gathers/DB/pokemon.db
    restart: unless-stopped

  gathers-webui:
    image: ghcr.io/morosanmihail/gathers-webui:latest
    ports:
      - "3000:3000"
    depends_on:
      - gathers-api
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "wget -q -O /dev/null http://127.0.0.1:3000/ || exit 1"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s

volumes:
  gathers-data:
```

## Using Docker Compose (Recommended)

1. Save the configuration above as `docker-compose.yml` and start the server:

   ```bash
   docker-compose up -d
   ```

2. The MTG database will be auto-downloaded on first start if not already present.

3. The web UI will be available at `http://localhost:3000`. The API server listens on port 5234.

4. To stop the server:
   ```bash
   docker-compose down
   ```

## Database Persistence

The Docker setup uses a named volume (`gathers-data`) to persist all databases across container restarts.

## Environment Variables

The Docker container supports the following environment variables (all set by default in docker-compose.yml):

| Variable | Default (in container) | Description |
|---|---|---|
| `GATHERS_SYSTEMS` | `riftbound-sql,scryfall` | Comma-separated list of systems to enable. Valid values: `scryfall`, `sql`, `riftbound-sql`, `pokemon-sql` |
| `MTG_DB_PATH` | `/home/app/.local/share/gathers/DB/AllPrintings.db` | MTG SQLite database |
| `MTG_PRICES_PATH` | `/home/app/.local/share/gathers/DB/AllPricesToday.json` | MTG price data |
| `RIFTBOUND_DB_PATH` | `/home/app/.local/share/gathers/DB/riftbound.db` | Riftbound SQLite database |
| `POKEMON_DB_PATH` | `/home/app/.local/share/gathers/DB/pokemon.db` | Pokémon SQLite database |
| `STORAGE_DB_PATH` | `/home/app/.local/share/gathers/DB/storage.db` | User collection database |

## Default Configuration

On first start, the server auto-creates a TOML config file at:

```
/home/app/.local/share/gathers/server.toml
```

This file is stored inside the `gathers-data` named volume, so it persists across container restarts. A freshly generated config looks like:

```toml
system = ["riftbound-sql"]
port = 5234
pricing_enabled = true

mtg_db_path = "/home/app/.local/share/gathers/DB/AllPrintings.db"
mtg_prices_path = "/home/app/.local/share/gathers/DB/AllPricesToday.json"
riftbound_db_path = "/home/app/.local/share/gathers/DB/riftbound.db"
pokemon_db_path = "/home/app/.local/share/gathers/DB/pokemon.db"
storage_db_path = "/home/app/.local/share/gathers/DB/storage.db"
```

### Editing the config

**Option 1 — environment variables (recommended for Docker):** set the variables in `docker-compose.yml`. They override the config file each session without editing it.

**Option 2 — edit the file directly:**

```bash
# Find the volume mount point
docker volume inspect gathers-data

# Or exec into the running container
docker exec -it <container-name> sh
vi /home/app/.local/share/gathers/server.toml
```

Then restart the container for changes to take effect:

```bash
docker-compose restart gathers-api
```

> **Note:** `pricing_enabled` can be toggled live via the Settings page in the web UI without a restart.

**Priority order** (highest wins): environment variables → `server.toml`.

The `system` field controls which card databases are active. Supported values: `scryfall`, `sql`, `riftbound-sql`, `pokemon-sql`. Multiple systems can be listed.

## Ports

- `3000`: Web UI
- `5234`: API server

## Building Manually

If you want to build the Docker image manually:

```bash
docker build -t gathers-server .
```

---

## Deploying via Stack Managers

Both tools below accept the compose config above — copy it in directly.

### Portainer Stacks

Portainer lets you deploy and manage Compose stacks through a web UI.

1. Open Portainer → **Stacks** → **Add stack**.
2. Paste the compose configuration above into the web editor, or point it at the Git repository URL.
3. Set any environment variable overrides in the **Environment variables** section if you want non-default paths.
4. Click **Deploy the stack**.

Updates: edit the stack in Portainer and redeploy, or enable **Auto update** with a polling interval if pulling from Git.

