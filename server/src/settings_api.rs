use aide::axum::{ApiRouter, routing::get};
use axum::{Json, extract::State, http::StatusCode};

use crate::{ErrorPayload, GathersState, ServerConfig};

fn demo_mode() -> bool {
    std::env::var("DEMO_MODE").is_ok()
}

fn demo_err() -> (StatusCode, Json<ErrorPayload>) {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorPayload {
            error: "Settings are disabled in demo mode".into(),
        }),
    )
}

pub fn settings_routes() -> ApiRouter<GathersState> {
    ApiRouter::new().api_route("/", get(get_settings).post(post_settings))
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
    Ok(Json(new_config))
}
