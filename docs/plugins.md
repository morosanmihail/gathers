# Writing a Plugin

A plugin is a separate HTTP service that supplies its own card-shaped catalog to a gatheRs server — without being compiled into it, and without needing to be written in Rust. The `dummy-plugin` crate in this repo is a complete, working reference implementation (it serves a hardcoded pool of books instead of trading cards, specifically to prove the contract isn't tied to Magic/Riftbound/Pokémon) — every example below points at its actual code.

## Why plugins aren't just another retrieval system

Internally, gatheRs represents a card as `models::Card` — a closed enum over `Magic`/`Riftbound`/`Pokemon`, matched exhaustively throughout collections, pricing, and search. Extending that enum for an arbitrary third-party domain (books, coins, whatever) would mean every one of those call sites has to know how to handle a domain it was never designed for.

So a plugin doesn't implement the internal `RetrievalSystemTrait` or return `models::Card`. Instead it speaks a small, deliberately generic JSON contract over HTTP, and the server talks to it through `retrieval::PluginRetrievalSystem` (see `retrieval/src/systems/plugin.rs`), which is just an HTTP client. Any number of plugins can be registered — each one is a `base_url` + `name` pair, not a new Rust type.

## The wire contract

A plugin implements four routes, rooted at whatever `base_url` you configure:

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/gathers-plugin/v1/info` | — | `PluginInfo` |
| POST | `/gathers-plugin/v1/search` | `PluginSearchRequest` | `Vec<PluginCard>` |
| POST | `/gathers-plugin/v1/cards/by-ids` | `Vec<String>` (ids) | `HashMap<String, PluginCard>` |
| POST | `/gathers-plugin/v1/update` | — | `PluginUpdateResponse` |

### `PluginCard`

The one shape you actually need to produce. Every field beyond `id`/`name` is optional-ish — an empty string, `null`, or an empty object are all valid, so you only fill in what makes sense for your domain:

```json
{
  "id": "book-1",
  "name": "Dune",
  "set_code": "SCIFI",
  "set_name": "Science Fiction",
  "collector_number": "1965",
  "description": "A young noble navigates politics and prophecy on a desert planet.",
  "image_url": "https://placehold.co/300x400/2d2d44/e8e8f0?font=roboto&text=Dune",
  "extra": { "author": "Frank Herbert" }
}
```

`set_code`/`set_name`/`collector_number` don't have to mean "set" and "collector number" literally — `dummy-plugin` uses them for genre code, genre name, and publication year (see `book()` in `dummy-plugin/src/main.rs`). `extra` is a freeform `string -> string` bag for anything else domain-specific; gatheRs never looks inside it.

### `GET /gathers-plugin/v1/info`

```json
{ "name": "dummy-plugin", "version": "0.1.0", "capabilities": ["search", "update"] }
```

`capabilities` isn't enforced by the server today, but declare what you actually support — it's the natural place for a future gatheRs version to skip calling endpoints you don't implement (`update` in particular is fine to no-op, see below).

### `POST /gathers-plugin/v1/search`

Request:

```json
{ "filters": { "text": "dune", "set_code": "SCIFI" }, "skip": 0, "limit": 24 }
```

`filters.text` and `filters.set_code` are both optional and both nullable. gatheRs' own search UI maps its "Name" and "Rules text" fields into `text`, and its set-code field into `set_code`. `dummy-plugin`'s implementation (`search()` in `dummy-plugin/src/main.rs`) matches `text` case-insensitively as a substring against both `name` and `description` — `LIKE '%text%'`, not an exact match — and `set_code` case-insensitively too. That's a convention worth following: gatheRs' own search box expects substring matching, not exact.

Response: `Vec<PluginCard>` — the page of results after `skip`/`limit` have been applied. Pagination is entirely your responsibility; gatheRs sends the same `skip`/`limit` semantics it uses for its own search endpoints.

### `POST /gathers-plugin/v1/cards/by-ids`

Request: a plain JSON array of ids — `["book-1", "book-4"]`.

Response: `{ "book-1": { ...PluginCard }, "book-4": { ...PluginCard } }`. An id you don't recognize should just be omitted from the response, not an error — this is what gatheRs uses to hydrate cards already added to a collection, and to resolve provider ownership when a card is first added (see [What plugins get for free](#what-plugins-get-for-free) below).

### `POST /gathers-plugin/v1/update`

```json
{ "started": true }
```

**Respond immediately.** gatheRs' own `/api/{mtg,pokemon,riftbound}/update` endpoints used to block on the actual download/scrape inline, and would get silently cancelled by the server's global 10-second request timeout on anything slower than that — a real, previously-shipped bug. Don't repeat it: if your plugin's "update" does real work (re-scraping a source, rebuilding a database), kick it off in the background and return `{"started": true}` right away. `dummy-plugin` has nothing to refresh (its pool is hardcoded), so it just logs and returns `{"started": true}` unconditionally — see `update()` in `dummy-plugin/src/main.rs`.

## Registering a plugin

Add a `[[plugins]]` block to the server's `server.toml` (any number of these):

```toml
[[plugins]]
name = "dummy-books"
base_url = "http://localhost:5236"
enabled = true
```

- `name` is how the plugin is addressed everywhere: `/api/plugins/{name}/...`, and the provider string `plugin-{name}` used when one of its cards is stored in a collection. The provider string is always stored lowercase (a plugin named `Books` stores `plugin-books`), and name lookups ignore case; the name is displayed as you configured it.
- Config changes take effect on server restart, same as `system`/`auto_download_*`.
- You can also add/remove `[[plugins]]` entries from the web UI's Settings page (under the Systems panel) instead of hand-editing the file. That page also has an **Update** button per enabled plugin, which calls your `/gathers-plugin/v1/update` — so implement it, even as a no-op, rather than leaving it unreachable.
- Two plugins can't share a `name`: the Settings page blocks saving a config with a duplicate, and the server also logs a warning (keeping only the last one) if it finds duplicates in a hand-edited `server.toml`.

## What plugins get for free

Once registered, `name` shows up in `/api/system`'s `plugins` list, and the web UI's search page/add-to-collection modal automatically render it as a selectable source — no frontend changes needed. Cards can be searched, viewed, and **added to a real collection**: `POST /api/collection/cards/{id}/add` probes your plugin's `cards/by-ids` the same way it probes MTG/Riftbound/Pokémon, and stores the card with `provider = "plugin-{name}"` (lowercased).

What that gets you, concretely, once a card is in a collection:
- It shows up in the plain collection listing and count.
- Search-within-collection and card-level sort (Name/SetCode/CollectorNumber) work against it, using only the fields `PluginCard` actually has — filters your card can't satisfy (rarity, colors, mana value, ...) are treated as "not applicable" rather than excluding it.
- It renders on shared/public collection pages, using `image`/`setCode`/`collectorNumber` (camelCase) — the same field names the rest of the app's card rendering already expects.
- If you disable the plugin later, its cards stop showing up in the web UI (both the plain collection view and search-within-collection) and on public share pages — the raw entries remain in storage, so re-enabling the plugin brings them back. The raw `/api/collection/cards/{id}/list` API response itself isn't filtered (that endpoint intentionally skips all card-detail hydration for speed); hiding happens in the web UI client and in every endpoint that already does hydration.

What it doesn't get: pricing (no `get_card_prices` equivalent exists in the contract — a plugin card always contributes $0 to collection value), rarity-aware sorting (plugin cards sort last), and Artist/ReleaseDate sort fields (blank for plugin cards, same as any system that doesn't have that concept).

## Running `dummy-plugin` locally

```bash
cargo run -p dummy-plugin
```

Listens on port `5236` by default (override with `DUMMY_PLUGIN_PORT`). It's also wired into the repo's `Tiltfile` as its own `dummy-plugin` resource, so `tilt up` starts it alongside `server`/`webui2` automatically.

Smoke-test it directly, no gatheRs server required:

```bash
curl http://localhost:5236/gathers-plugin/v1/info
curl -X POST http://localhost:5236/gathers-plugin/v1/search \
  -H 'content-type: application/json' \
  -d '{"filters":{"text":"dune"}}'
```

Then point a gatheRs server at it via the `[[plugins]]` block above and confirm it end-to-end:

```bash
curl http://localhost:5234/api/system                                  # "dummy-books" in "plugins"
curl -X POST http://localhost:5234/api/plugins/dummy-books/search \
  -H 'content-type: application/json' -d '{"filters":{}}'
```

## Writing your own

You don't need Rust, or this repo, to write a plugin — only something that can serve those four JSON routes. Pick whatever language/framework you like, model `PluginCard` for your domain (borrow `dummy-plugin`'s `book()` helper as a template for "one function per item, called from a fixed list" if your data is small and static; reach for a real database if it isn't), implement `search`'s filtering as substring/case-insensitive matches on whatever fields make sense, and make `update` non-blocking. Then register it and it behaves like any other source in the app.
