use criterion::{Criterion, criterion_group, criterion_main};
use persistence::{PersistenceSystem, SQLitePersistenceSystem};
use retrieval::{PokemonSQLiteRetrievalSystem, RetrievalSystem};
use std::{hint::black_box, time::Instant};

fn bench_csv_import(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pokemon_db = std::env::var("POKEMON_DB_PATH")
        .unwrap_or_else(|_| "../data/pokemon.db".to_string());
    let pokemon_prices = std::env::var("POKEMON_PRICES_PATH").ok();
    let pokemon_csv = std::env::var("POKEMON_CSV_PATH")
        .unwrap_or_else(|_| "../data/pokemon_test.csv".to_string());
    let retrieval = RetrievalSystem::PokemonSQLiteRetrievalSystem(
        PokemonSQLiteRetrievalSystem::new(Some(pokemon_db), pokemon_prices).unwrap(),
    );

    let mut group = c.benchmark_group("csv_import");

    group.bench_function("import_pokemon_csv", |b| {
        b.to_async(&rt).iter(|| async {
            let mut persistence = PersistenceSystem::SQLitePersistenceSystem(
                SQLitePersistenceSystem::new(true, None).unwrap(),
            );
            let start = Instant::now();
            let result = persistence
                .import_csv(
                    black_box(pokemon_csv.clone()),
                    "Pokemon Import".to_string(),
                    &[retrieval.clone()],
                    None,
                )
                .await;
            let duration = start.elapsed();
            let _ = black_box(result);
            duration
        })
    });

    group.finish();
}

criterion_group!(benches, bench_csv_import);
criterion_main!(benches);
