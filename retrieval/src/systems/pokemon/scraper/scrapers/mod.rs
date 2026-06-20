pub mod serebii;
pub mod tcgp;

use std::time::Duration;

use eyre::Result;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{info, warn};

use crate::systems::pokemon::scraper::db::{Db, UpdateFields};
use crate::systems::pokemon::scraper::models::Expansion;

pub struct RunOptions {
    pub recent: Option<usize>,
}

pub async fn run_all(db: &Db, client: &reqwest::Client, opts: &RunOptions) -> Result<()> {
    info!("Loading TCGPlayer set index");
    let tcgp_sets = tcgp::get_tcgp_sets(client).await.unwrap_or_else(|e| {
        warn!("Failed to load TCGP sets: {e}");
        vec![]
    });

    info!("Loading TCGPlayer set codes");
    let tcgp_codes = tcgp::get_tcgp_codes(client).await.unwrap_or_else(|e| {
        warn!("Failed to load TCGP codes: {e}");
        vec![]
    });

    info!("Fetching normal sets");
    let normal_sets = serebii::scrape_normal_sets(client, opts.recent)
        .await
        .unwrap_or_default();

    info!("Fetching promo sets");
    let promo_sets = serebii::scrape_promo_sets(client, opts.recent)
        .await
        .unwrap_or_default();

    let all_sets: Vec<_> = normal_sets.into_iter().chain(promo_sets).collect();

    info!(count = all_sets.len(), "Updating cards");
    let pb = ProgressBar::new(all_sets.len() as u64);
    pb.set_style(
        ProgressStyle::with_template("[{pos}/{len}] {wide_msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    let mut skipped = 0u64;
    for set in &all_sets {
        pb.set_message(set.name.clone());

        if db.has_set(&set.name) {
            skipped += 1;
            pb.inc(1);
            continue;
        }

        let tcgp_names = tcgp::find_set_from_tcgp(&set.name, &tcgp_sets);
        let tcg_name_json = serde_json::to_string(&tcgp_names).unwrap_or_else(|_| "[]".into());
        let exp = Expansion {
            name: set.name.clone(),
            tcg_name: tcg_name_json,
            ..Default::default()
        };

        if let Err(e) = update_cards(db, client, &exp, &set.page, &tcgp_codes).await {
            warn!("Failed to update cards for '{}': {e}", set.name);
        }
        pb.inc(1);
    }
    pb.finish_with_message("Done");
    if skipped > 0 {
        info!(skipped, "Skipped sets already present in db");
    }

    Ok(())
}

async fn update_cards(
    db: &Db,
    client: &reqwest::Client,
    exp: &Expansion,
    serebii_page: &str,
    tcgp_codes: &[crate::systems::pokemon::scraper::models::TcgpCode],
) -> Result<()> {
    info!("Updating cards for '{}'", exp.name);

    let (_, serebii_cards) = serebii::scrape_set_details(client, serebii_page, &exp.name)
        .await
        .unwrap_or_else(|e| {
            warn!("Serebii card fetch failed for '{}': {e}", exp.name);
            (String::new(), vec![])
        });

    let tcgp_cards = tcgp::pull_set_cards(client, exp, tcgp_codes)
        .await
        .unwrap_or_else(|e| {
            warn!("TCGP card fetch failed for '{}': {e}", exp.name);
            vec![]
        });

    for card in serebii_cards {
        db.upsert_card(&card, UpdateFields::Serebii);

        if tcgp_cards.is_empty() {
            if let Some(mut enriched) =
                tcgp::search_card(client, &card.name, &exp.name, tcgp_codes).await
            {
                enriched.img = card.img.clone();
                db.upsert_card(&enriched, UpdateFields::Tcgp);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    for card in tcgp_cards {
        db.upsert_card(&card, UpdateFields::Tcgp);
    }

    Ok(())
}
