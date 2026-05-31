# Running Benchmarks

GatheRs uses [Criterion](https://github.com/bheisler/criterion.rs) for benchmarking. Two benchmark suites are available:

| Suite | File | What it measures |
|---|---|---|
| `card_search_benchmark` | `benches/src/card_search_benchmark.rs` | MTG SQLite search queries (name, color, rarity, etc.) |
| `csv_import_benchmark` | `benches/src/csv_import_benchmark.rs` | Pokémon CSV collection import |

## Running

Run all benchmarks:

```bash
cargo bench
```

Run a specific suite:

```bash
cargo bench --bench card_search_benchmark
cargo bench --bench csv_import_benchmark
```

Run only benchmarks matching a filter:

```bash
cargo bench --bench card_search_benchmark -- search_by_name
```

## Specifying Database Paths

By default the benchmarks use the paths in the table below. Override any of them by setting the corresponding environment variable before the command.

| Variable | Default | Used by |
|---|---|---|
| `MTG_DB_PATH` | `../data/testPrintings.db` | `card_search_benchmark` |
| `MTG_PRICES_PATH` | *(none)* | `card_search_benchmark` |
| `POKEMON_DB_PATH` | `../data/pokemon.db` | `csv_import_benchmark` |
| `POKEMON_PRICES_PATH` | *(none)* | `csv_import_benchmark` |
| `POKEMON_CSV_PATH` | `../data/pokemon_test.csv` | `csv_import_benchmark` |

Examples:

```bash
# Use the production MTG database
MTG_DB_PATH=~/.local/share/gathers/DB/AllPrintings.db cargo bench --bench card_search_benchmark

# Use a custom Pokémon database and CSV
POKEMON_DB_PATH=/data/pokemon.db POKEMON_CSV_PATH=/data/my_collection.csv cargo bench --bench csv_import_benchmark

# Multiple overrides
MTG_DB_PATH=/path/to/AllPrintings.db MTG_PRICES_PATH=/path/to/AllPricesToday.sqlite cargo bench
```

## Output

Criterion writes HTML reports to `target/criterion/`. Open `target/criterion/report/index.html` in a browser to view results with charts.

To compare against a saved baseline:

```bash
# Save current results as baseline
cargo bench -- --save-baseline my_baseline

# Compare against it later
cargo bench -- --baseline my_baseline
```
