use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::{Json, extract::State, http::StatusCode};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{ErrorPayload, GathersState, RESTART, ServerConfig, demo_mode, demo_err};

pub fn settings_routes() -> ApiRouter<GathersState> {
    ApiRouter::new()
        .api_route("/", get(get_settings).post(post_settings))
        .api_route("/restart", post(restart_server))
}

#[derive(Debug, Serialize, JsonSchema)]
struct RestartResponse {
    /// Always true: the server is shutting down to re-exec itself.
    restarting: bool,
}

/// Restarts the server in place (same binary, args and environment). The
/// response is sent before the process goes down; clients should then poll
/// until the server answers again.
async fn restart_server() -> Result<Json<RestartResponse>, (StatusCode, Json<ErrorPayload>)> {
    if demo_mode() {
        return Err(demo_err());
    }
    tracing::info!("Restart requested via settings API");
    RESTART.notify_waiters();
    Ok(Json(RestartResponse { restarting: true }))
}

async fn get_settings(
    State(state): State<GathersState>,
) -> Result<Json<ServerConfig>, (StatusCode, Json<ErrorPayload>)> {
    if demo_mode() {
        return Err(demo_err());
    }
    let ret = state.0.lock().await;
    let config_path = ret.config_path.clone();
    drop(ret);
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload { error: format!("Failed to read config: {e}") }),
        )
    })?;
    let config: ServerConfig = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload { error: format!("Failed to parse config: {e}") }),
        )
    })?;
    Ok(Json(config))
}

async fn post_settings(
    State(state): State<GathersState>,
    Json(new_config): Json<ServerConfig>,
) -> Result<Json<ServerConfig>, (StatusCode, Json<ErrorPayload>)> {
    if demo_mode() {
        return Err(demo_err());
    }
    let ret = state.0.lock().await;
    let config_path = ret.config_path.clone();
    drop(ret);
    // If the current config can't be read, assume the worst.
    let restart_needed = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str::<ServerConfig>(&content).ok())
        .is_none_or(|old| old.restart_needed(&new_config));
    let toml_str = toml::to_string_pretty(&new_config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload { error: format!("Failed to serialize config: {e}") }),
        )
    })?;
    std::fs::write(&config_path, &toml_str).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload { error: format!("Failed to write config: {e}") }),
        )
    })?;
    let mut ret = state.0.lock().await;
    ret.pricing_enabled = new_config.pricing_enabled;
    ret.collections_enabled = new_config.collections_enabled;
    ret.restart_required |= restart_needed;
    drop(ret);
    Ok(Json(new_config))
}
