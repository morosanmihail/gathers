use axum::Json;
use axum::http::StatusCode;
use models::Set;
use retrieval::{RetrievalSystem, RetrievalSystemTrait as _};
use schemars::JsonSchema;
use serde::Serialize;

use crate::collections::collections_models::APIUniqueMode;
use crate::{ApiError, ErrorPayload};

/// Everything a game's search can be narrowed by, in one call: the values its
/// set, card type and grouping filters accept.
#[derive(Serialize, JsonSchema)]
pub struct APISystemMeta {
    pub sets: Vec<Set>,
    /// Values for `APICardSearchFilters::any_types`, sorted.
    pub card_types: Vec<String>,
    /// Values for `APICardSearchFilters::unique`, default first. Empty when
    /// the system has nothing to choose between.
    pub unique_modes: Vec<APIUniqueMode>,
}

/// The `/meta` route every game's API serves.
pub(crate) async fn system_meta(system: RetrievalSystem) -> Result<Json<APISystemMeta>, ApiError> {
    let meta = system.get_meta().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                error: format!("Failed to get system metadata. {e}"),
            }),
        )
    })?;
    Ok(Json(APISystemMeta {
        sets: meta.sets,
        card_types: meta.card_types,
        unique_modes: meta.unique_modes.into_iter().map(Into::into).collect(),
    }))
}
