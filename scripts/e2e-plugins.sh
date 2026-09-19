#!/usr/bin/env bash
#
# Runs the plugin-provider-resolution e2e test (e2e/examples/plugin_provider_resolution.rs)
# against a fully isolated, ephemeral server + dummy-plugin pair:
#   - a scratch $HOME so the server writes its own server.toml/DB under a
#     temp directory instead of ~/.local/share/gathers — your real config
#     and collections are never touched
#   - both processes bound to non-default ports, so this never collides
#     with a server/dummy-plugin already running normally (e.g. via
#     `tilt up`) on 5234/5236
#   - both processes are torn down and the scratch dir removed on exit,
#     success or failure
#
# Run via `just e2e-plugins`, or directly: ./scripts/e2e-plugins.sh

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

SERVER_PORT=15234
PLUGIN_PORT=15236
TMP_HOME="$(mktemp -d)"

SERVER_PID=""
PLUGIN_PID=""

cleanup() {
    local status=$?
    [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true
    [[ -n "$PLUGIN_PID" ]] && kill "$PLUGIN_PID" 2>/dev/null || true
    [[ -n "$SERVER_PID" ]] && wait "$SERVER_PID" 2>/dev/null || true
    [[ -n "$PLUGIN_PID" ]] && wait "$PLUGIN_PID" 2>/dev/null || true
    rm -rf "$TMP_HOME"
    exit "$status"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP_HOME/.local/share/gathers"
cat > "$TMP_HOME/.local/share/gathers/server.toml" <<EOF
system = []
port = $SERVER_PORT
pricing_enabled = false
collections_enabled = true
auto_download_enabled = false
auto_download_interval_hours = 24
storage_db_path = "$TMP_HOME/storage.db"

[[plugins]]
name = "books-a"
base_url = "http://localhost:$PLUGIN_PORT"
enabled = true

[[plugins]]
name = "books-b"
base_url = "http://localhost:$PLUGIN_PORT"
enabled = true
EOF

echo "Building server, dummy-plugin, and e2e examples..."
cargo build -p server -p dummy-plugin -p e2e --examples

DUMMY_PLUGIN_PORT=$PLUGIN_PORT ./target/debug/dummy-plugin &
PLUGIN_PID=$!

HOME="$TMP_HOME" GATHERS_NO_AUTO_UPDATE=1 ./target/debug/server &
SERVER_PID=$!

wait_for() {
    local url=$1 name=$2
    for _ in $(seq 1 100); do
        curl -sf -o /dev/null "$url" && return 0
        sleep 0.1
    done
    echo "error: $name never came up at $url" >&2
    return 1
}

wait_for "http://localhost:$PLUGIN_PORT/gathers-plugin/v1/info" "dummy-plugin"
wait_for "http://localhost:$SERVER_PORT/api/system" "server"

GATHERS_URL="http://localhost:$SERVER_PORT" cargo run -p e2e --example plugin_provider_resolution
