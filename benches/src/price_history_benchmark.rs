//! Price history persistence: recording daily snapshots, reading a card's
//! history back and listing the cards collections track. Synthetic data in
//! in-memory databases, so no downloaded card or price DBs are needed.

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use models::CollectionCard;
use persistence::{PersistenceSystemTrait, PricePoint, SQLitePersistenceSystem};
use std::hint::black_box;

const PROVIDER: &str = "MagicSQLite";
const RETAILERS: [&str; 2] = ["cardkingdom", "cardmarket"];
const FINISHES: [&str; 2] = ["", "foil"];

/// One price per retailer and finish for each of `cards` cards.
fn snapshot(cards: usize, day: usize) -> Vec<PricePoint> {
    (0..cards)
        .flat_map(|card| {
            RETAILERS.iter().flat_map(move |retailer| {
                FINISHES.iter().map(move |finish| PricePoint {
                    card_uuid: format!("card-{card}"),
                    retailer: retailer.to_string(),
                    finish: finish.to_string(),
                    price: 1.0 + (card % 100) as f64 + day as f64 / 100.0,
                    currency: "EUR".to_string(),
                    recorded_on: date(day),
                })
            })
        })
        .collect()
}

fn date(n: usize) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Days::new(n as u64)
}

fn with_history() -> SQLitePersistenceSystem {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    p.enable_price_history(true, None).unwrap();
    p
}

fn bench_record(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("price_history_record");
    group.sample_size(20);

    for cards in [1usize, 1_000, 10_000] {
        let points = snapshot(cards, 0);

        // A day's first snapshot: every row is new.
        group.bench_with_input(BenchmarkId::new("new_day", cards), &points, |b, points| {
            b.iter_batched(
                with_history,
                |mut p| rt.block_on(async { black_box(p.record_prices(PROVIDER, points).await.unwrap()) }),
                BatchSize::LargeInput,
            )
        });

        // A later snapshot the same day: every row is an upsert.
        let populated = with_history();
        rt.block_on(async { populated.clone().record_prices(PROVIDER, &points).await.unwrap() });
        group.bench_with_input(BenchmarkId::new("same_day", cards), &points, |b, points| {
            b.iter(|| rt.block_on(async { black_box(populated.clone().record_prices(PROVIDER, points).await.unwrap()) }))
        });
    }
    group.finish();
}

fn bench_read(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("price_history_read");

    // A year of daily snapshots of 1000 cards.
    let mut p = with_history();
    rt.block_on(async {
        for d in 0..365 {
            p.record_prices(PROVIDER, &snapshot(1_000, d)).await.unwrap();
        }
    });

    group.bench_function("get_card_year", |b| {
        b.iter(|| rt.block_on(async { black_box(p.get_price_history(PROVIDER, &"card-500".to_string()).await.unwrap()) }))
    });
    group.bench_function("has_price_history", |b| {
        b.iter(|| rt.block_on(async { black_box(p.has_price_history(PROVIDER).await.unwrap()) }))
    });
    group.bench_function("has_price_history_other_provider", |b| {
        b.iter(|| rt.block_on(async { black_box(p.has_price_history("PokemonSQLite").await.unwrap()) }))
    });
    group.finish();
}

fn bench_tracked(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("price_history_tracked_cards");

    for cards in [1_000usize, 10_000] {
        // Spread over 5 collections, with each card in two of them and in
        // two finishes, so DISTINCT has duplicates to fold.
        let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
        rt.block_on(async {
            for col in 0..5 {
                let name = p.add_collection(format!("col-{col}")).await.unwrap();
                let rows: Vec<CollectionCard> = (0..cards)
                    .filter(|card| card % 5 == col || (card + 1) % 5 == col)
                    .flat_map(|card| {
                        let name = name.clone();
                        FINISHES.iter().map(move |finish| CollectionCard {
                            uuid: format!("card-{card}"),
                            finish: finish.to_string(),
                            quantity: 1,
                            want_quantity: 0,
                            time_added: "2026-01-01T00:00:00Z".to_string(),
                            collection: name.clone(),
                            provider: PROVIDER.to_string(),
                        })
                    })
                    .collect();
                for chunk in rows.chunks(500) {
                    p.add_cards_to_collection(&name, chunk).await.unwrap();
                }
            }
        });

        group.bench_with_input(BenchmarkId::from_parameter(cards), &p, |b, p| {
            b.iter(|| rt.block_on(async { black_box(p.tracked_card_uuids(PROVIDER).await.unwrap()) }))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_record, bench_read, bench_tracked);
criterion_main!(benches);
