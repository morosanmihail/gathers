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
| `MTG_DB_PATH` | `/home/app/.local/share/gathers/DB/AllPrintings.db` | MTG SQLite database |
| `RIFTBOUND_DB_PATH` | `/home/app/.local/share/gathers/DB/riftbound.db` | Riftbound SQLite database |
| `POKEMON_DB_PATH` | `/home/app/.local/share/gathers/DB/pokemon.db` | Pokémon SQLite database |
| `STORAGE_DB_PATH` | `/home/app/.local/share/gathers/DB/storage.db` | User collection database |

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

