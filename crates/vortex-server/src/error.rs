use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::cache::CacheError;

#[derive(Debug)]
pub enum AppError {
    /// Configuracion no encontrada
    NotFound { app: String, profile: String },

    /// Parametros invalidos
    BadRequest(String),

    /// Error interno
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            AppError::NotFound { app, profile } => (
                StatusCode::NOT_FOUND,
                "Not Found",
                format!("Configuration not found for {}/{}", app, profile),
            ),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "Bad Request", msg),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                msg,
            ),
        };

        let body = Json(ErrorResponse {
            error: error.to_string(),
            message,
        });

        (status, body).into_response()
    }
}

impl From<CacheError> for AppError {
    fn from(err: CacheError) -> Self {
        AppError::Internal(err.to_string())
    }
}
