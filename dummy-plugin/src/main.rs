//! Example third-party gathers plugin: serves a hardcoded pool of books
//! instead of trading cards, to prove the plugin contract isn't tied to
//! any particular card game. Implements the wire contract documented in
//! `retrieval::systems::plugin` — a real plugin can be written in any
//! language as long as it speaks this same JSON-over-HTTP shape.
//!
//! Run it, then point a gathers server at it via `server.toml`:
//!
//! ```toml
//! [[plugins]]
//! name = "dummy-books"
//! base_url = "http://localhost:5236"
//! ```

use std::collections::HashMap;

use axum::{
    Json, Router,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize)]
struct PluginCard {
    id: String,
    name: String,
    #[serde(default)]
    set_code: String,
    #[serde(default)]
    set_name: String,
    #[serde(default)]
    collector_number: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    extra: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchFilters {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    set_code: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchRequest {
    #[serde(default)]
    filters: SearchFilters,
    #[serde(default)]
    skip: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct PluginInfo {
    name: String,
    version: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
struct UpdateResponse {
    started: bool,
}

/// Percent-encodes everything but unreserved characters — enough for a
/// query-string value, no crate needed for this small, fixed use.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A generated "cover" — no real artwork, just the title on a solid card,
/// same role Scryfall image URLs play for real MTG cards (see
/// `cardImageUrl` in webui2). Uses an external placeholder-image service
/// rather than shipping an image-generation dependency for a dummy plugin.
fn cover_image_url(name: &str) -> String {
    format!("https://placehold.co/300x400/2d2d44/e8e8f0?font=roboto&text={}", url_encode(name))
}

fn book(id: &str, name: &str, genre_code: &str, genre_name: &str, year: &str, author: &str, blurb: &str) -> PluginCard {
    PluginCard {
        id: id.to_string(),
        name: name.to_string(),
        set_code: genre_code.to_string(),
        set_name: genre_name.to_string(),
        collector_number: year.to_string(),
        description: Some(blurb.to_string()),
        image_url: Some(cover_image_url(name)),
        extra: HashMap::from([("author".to_string(), author.to_string())]),
    }
}

/// The entire "database" — a fixed pool, regenerated on every call instead
/// of loaded from disk, since a real plugin's actual storage is none of
/// gathers' business.
fn books() -> Vec<PluginCard> {
    vec![
        book("book-1", "Dune", "SCIFI", "Science Fiction", "1965", "Frank Herbert", "A young noble navigates politics and prophecy on a desert planet."),
        book("book-2", "Foundation", "SCIFI", "Science Fiction", "1951", "Isaac Asimov", "A mathematician predicts the fall of a galactic empire."),
        book("book-3", "Neuromancer", "SCIFI", "Science Fiction", "1984", "William Gibson", "A washed-up hacker is hired for one last job in cyberspace."),
        book("book-4", "The Hobbit", "FANTASY", "Fantasy", "1937", "J.R.R. Tolkien", "A reluctant burglar joins dwarves on a quest to reclaim their mountain."),
        book("book-5", "A Wizard of Earthsea", "FANTASY", "Fantasy", "1968", "Ursula K. Le Guin", "A young mage must undo a shadow he unleashed on the world."),
        book("book-6", "Mistborn", "FANTASY", "Fantasy", "2006", "Brandon Sanderson", "A street urchin discovers she can rule the ashes of a fallen empire."),
        book("book-7", "Sapiens", "NONFICTION", "Non-Fiction", "2011", "Yuval Noah Harari", "A survey of how Homo sapiens came to dominate the planet."),
        book("book-8", "Cosmos", "NONFICTION", "Non-Fiction", "1980", "Carl Sagan", "A tour of the universe and humanity's place within it."),
        book("book-9", "And Then There Were None", "MYSTERY", "Mystery", "1939", "Agatha Christie", "Ten strangers on an island, and a killer among them."),
        book("book-10", "The Hound of the Baskervilles", "MYSTERY", "Mystery", "1902", "Arthur Conan Doyle", "Sherlock Holmes investigates a supposedly supernatural hound."),
    ]
}

async fn info() -> Json<PluginInfo> {
    Json(PluginInfo {
        name: "dummy-plugin".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec!["search".to_string(), "update".to_string()],
    })
}

async fn search(Json(req): Json<SearchRequest>) -> Json<Vec<PluginCard>> {
    let text = req.filters.text.map(|t| t.to_lowercase());
    let matches_text = |card: &PluginCard| match &text {
        None => true,
        Some(t) => {
            card.name.to_lowercase().contains(t)
                || card
                    .description
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(t)
        }
    };
    let set_code = req.filters.set_code.map(|s| s.to_lowercase());
    let matches_set = |card: &PluginCard| match &set_code {
        None => true,
        Some(s) => card.set_code.to_lowercase() == *s,
    };

    let mut results: Vec<PluginCard> = books()
        .into_iter()
        .filter(|c| matches_text(c) && matches_set(c))
        .collect();

    let skip = req.skip.unwrap_or(0).min(results.len());
    results.drain(..skip);
    if let Some(limit) = req.limit {
        results.truncate(limit);
    }

    Json(results)
}

async fn cards_by_ids(Json(ids): Json<Vec<String>>) -> Json<HashMap<String, PluginCard>> {
    let pool = books();
    let found = ids
        .into_iter()
        .filter_map(|id| pool.iter().find(|b| b.id == id).cloned().map(|b| (id, b)))
        .collect();
    Json(found)
}

async fn update() -> Json<UpdateResponse> {
    tracing::info!("Update requested — hardcoded pool has nothing to refresh, acknowledging immediately");
    Json(UpdateResponse { started: true })
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let port: u16 = std::env::var("DUMMY_PLUGIN_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5236);

    let app = Router::new()
        .route("/gathers-plugin/v1/info", get(info))
        .route("/gathers-plugin/v1/search", post(search))
        .route("/gathers-plugin/v1/cards/by-ids", post(cards_by_ids))
        .route("/gathers-plugin/v1/update", post(update));

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "Dummy plugin serving a hardcoded pool of books");
    axum::serve(listener, app).await
}
