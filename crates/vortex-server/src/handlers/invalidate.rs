//! Cache invalidation endpoint handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::error::AppError;
use crate::state::AppState;

/// Response para operaciones de invalidación.
#[derive(Debug, Serialize)]
pub struct InvalidateResponse {
    /// Número de entries invalidadas.
    pub invalidated: usize,
    /// Mensaje descriptivo.
    pub message: String,
}

/// Request body para invalidación por patrones múltiples.
#[derive(Debug, Deserialize)]
pub struct InvalidateByPatternsRequest {
    /// Lista de patrones glob a invalidar.
    pub patterns: Vec<String>,
}

/// DELETE /cache
/// Invalida toda la cache.
#[instrument(skip_all)]
pub async fn invalidate_all(State(state): State<AppState>) -> Result<Response, AppError> {
    match state.cache() {
        Some(cache) => {
            let count = cache.entry_count();
            cache.invalidate_all();

            tracing::info!(count = count, "All cache entries invalidated");

            Ok((
                StatusCode::OK,
                Json(InvalidateResponse {
                    invalidated: count as usize,
                    message: format!("Invalidated all {} cache entries", count),
                }),
            )
                .into_response())
        },
        None => Err(AppError::Internal("Cache is not enabled".to_string())),
    }
}

/// DELETE /cache/{app}
/// Invalida todas las entries para una aplicación.
#[instrument(skip_all, fields(app = %path.app))]
pub async fn invalidate_by_app(
    State(state): State<AppState>,
    Path(path): Path<AppPath>,
) -> Result<Response, AppError> {
    match state.cache() {
        Some(cache) => {
            let result = cache.invalidate_by_app(&path.app).await;

            tracing::info!(
                app = %path.app,
                count = result.count,
                "Cache entries invalidated"
            );

            Ok((
                StatusCode::OK,
                Json(InvalidateResponse {
                    invalidated: result.count,
                    message: format!(
                        "Invalidated {} cache entries for app '{}'",
                        result.count, path.app
                    ),
                }),
            )
                .into_response())
        },
        None => Err(AppError::Internal("Cache is not enabled".to_string())),
    }
}

/// DELETE /cache/{app}/{profile}
/// Invalida todas las entries para una aplicación y perfil.
#[instrument(skip_all, fields(app = %path.app, profile = %path.profile))]
pub async fn invalidate_by_app_profile(
    State(state): State<AppState>,
    Path(path): Path<AppProfilePath>,
) -> Result<Response, AppError> {
    match state.cache() {
        Some(cache) => {
            let result = cache
                .invalidate_by_app_profile(&path.app, &path.profile)
                .await;

            tracing::info!(
                app = %path.app,
                profile = %path.profile,
                count = result.count,
                "Cache entries invalidated"
            );

            Ok((
                StatusCode::OK,
                Json(InvalidateResponse {
                    invalidated: result.count,
                    message: format!(
                        "Invalidated {} cache entries for app '{}' and profile '{}'",
                        result.count, path.app, path.profile
                    ),
                }),
            )
                .into_response())
        },
        None => Err(AppError::Internal("Cache is not enabled".to_string())),
    }
}

/// DELETE /cache/{app}/{profile}/{label}
/// Invalida una entry específica.
#[instrument(skip_all, fields(
    app = %path.app,
    profile = %path.profile,
    label = %path.label
))]
pub async fn invalidate_by_app_profile_label(
    State(state): State<AppState>,
    Path(path): Path<AppProfileLabelPath>,
) -> Result<Response, AppError> {
    match state.cache() {
        Some(cache) => {
            let result = cache
                .invalidate_by_app_profile_label(&path.app, &path.profile, &path.label)
                .await;

            tracing::info!(
                app = %path.app,
                profile = %path.profile,
                label = %path.label,
                "Cache entry invalidated"
            );

            Ok((
                StatusCode::OK,
                Json(InvalidateResponse {
                    invalidated: result.count,
                    message: format!(
                        "Invalidated cache entry for app '{}', profile '{}', label '{}'",
                        path.app, path.profile, path.label
                    ),
                }),
            )
                .into_response())
        },
        None => Err(AppError::Internal("Cache is not enabled".to_string())),
    }
}

// Path extractors

#[derive(Debug, Deserialize)]
pub struct AppPath {
    pub app: String,
}

#[derive(Debug, Deserialize)]
pub struct AppProfilePath {
    pub app: String,
    pub profile: String,
}

#[derive(Debug, Deserialize)]
pub struct AppProfileLabelPath {
    pub app: String,
    pub profile: String,
    pub label: String,
}
