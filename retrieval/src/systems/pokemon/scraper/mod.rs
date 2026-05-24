#![allow(dead_code)]

mod common;
mod db;
mod models;
mod scrapers;

use eyre::Result;
use db::Db;
use scrapers::{RunOptions, run_all};

pub struct Options {
    pub db_path: String,
    pub recent: Option<usize>,
    pub fresh: bool,
}

pub async fn run(opts: Options) -> Result<()> {
    if opts.fresh && std::path::Path::new(&opts.db_path).exists() {
        tracing::info!("Removing existing database: {}", opts.db_path);
        std::fs::remove_file(&opts.db_path)?;
    }

    let db = Db::open(&opts.db_path)?;

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; pokedata-scraper/1.0)")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    run_all(&db, &client, &RunOptions { recent: opts.recent }).await
}
