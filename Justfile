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

# Cut a release: set the workspace version, commit it and tag it (does not push). Usage: just release 0.7.1
release version:
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{version}}"
    v="${v#v}"
    if ! [[ "$v" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
        echo "error: '$v' is not a valid semver version (expected e.g. 0.7.1)" >&2
        exit 1
    fi
    if [ -n "$(git status --porcelain)" ]; then
        echo "error: working tree is not clean; commit or stash first" >&2
        exit 1
    fi
    if git rev-parse -q --verify "refs/tags/v$v" >/dev/null; then
        echo "error: tag v$v already exists" >&2
        exit 1
    fi
    # Every crate inherits `version.workspace = true`, so this is the only line to change.
    sed -i '0,/^version = ".*"/s//version = "'"$v"'"/' Cargo.toml
    # Refresh Cargo.lock for the workspace crates only (the Dockerfile builds with --locked).
    cargo update --workspace --offline
    git add Cargo.toml Cargo.lock
    git commit -m "Release v$v"
    git tag -a "v$v" -m "v$v"
    echo "Tagged v$v. Publish with: git push && git push origin v$v"
