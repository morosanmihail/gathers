# List available recipes
default:
    @just --list

# Run all workspace unit/integration tests
test:
    cargo test --workspace

# Run e2e tests (needs a live server — default http://localhost:5234, override with GATHERS_URL)
e2e:
    cargo run -p e2e --example collection_lifecycle
    cargo run -p e2e --example share_view
    cargo run -p e2e --example finishes

# Provider-resolution e2e test, self-contained: spins up an isolated temp server + dummy-plugin pair, runs the test, tears both down
e2e-plugins:
    ./scripts/e2e-plugins.sh

# Run criterion benchmarks
bench:
    cargo bench -p benches
