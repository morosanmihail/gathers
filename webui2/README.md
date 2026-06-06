# GatheRs Web UI v2

Svelte 5 / SvelteKit frontend for GatheRs. Requires the GatheRs server running on port 5234.

## Prerequisites

- Node.js 18+
- GatheRs server: `cargo run --bin server -- --port 5234`

## Setup

```bash
cd webui2
npm install
```

## Development

```bash
npm run dev
```

Opens at http://localhost:5173. API requests are proxied to http://localhost:5234.

## Production build

```bash
npm run build
```

Output goes to `build/`. Serve it as a static site alongside the GatheRs server, or point any static file server at the `build/` directory.

```bash
# Quick local preview of the production build
npm run preview
```
