# Gathers - HTMX Web UI

A lightweight, HTMX-based web interface for searching Magic: The Gathering cards using the Gathers backend API.

## Features

- **Card Search**: Search for cards by name, set code, collector number, artist, text, rarity, and color identity
- **Responsive Design**: Works on mobile and desktop devices
- **Fast Performance**: Uses HTMX for progressive enhancement without full page reloads
- **Clean Interface**: Simple, intuitive design focused on card searching

## Prerequisites

- Node.js 18+ (for development)
- Gathers backend server running (see [main README.md](../README.md))

## Installation

1. Install dependencies:
```bash
npm install
```

2. Build the application:
```bash
npm run build
```

3. The built files will be in the `dist` directory.

## Development

To run in development mode with hot reloading:

```bash
npm start
```

This will start a development server on port 3000 that proxies API requests to your Gathers backend (assumed to be running on port 8080).

## Configuration

The Vite configuration includes proxy settings for development. If you need to change the backend URL:

```javascript
// vite.config.js
server: {
  port: 3000,
  proxy: {
    '/mtg': 'http://localhost:8080' // Change this to your backend URL
  }
}
```

## API Endpoints Used

This web UI connects to the following Gathers backend endpoints:

- `POST /mtg/cards/search` - Search for cards with various filters
- `GET /mtg/sets` - Get available set codes (not implemented in current version)
- `GET /mtg/update` - Update card database (not implemented in current version)

## Supported Search Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| name | string | Card name (partial matches work) |
| set_code | string | Set code (e.g., "M21", "ZNR") |
| collector_number | string | Collector number in the set |
| artist | string | Card artist name |
| text | string | Card text/oracle text |
| rarity | string | Rarity: common, uncommon, rare, mythic |
| color_identities | array | Color identity letters (e.g., ["W", "U"]) |
| types | array | Card types (e.g., ["Creature"]) |
| subtypes | array | Card subtypes (e.g., ["Human", "Soldier"]) |
| supertypes | string | Card supertypes (e.g., "Legendary") |
| limit | number | Maximum number of results (default: 20) |

## Differences from React Version

This HTMX version is focused on card searching only and does not include:

- Collection management features
- User authentication
- Advanced pagination controls
- Complex state management

The goal is to provide a simple, fast interface for searching cards without the overhead of a full React application.

## License

See the main [LICENSE](../LICENSE) file.