//! Configuration endpoint handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use tracing::instrument;
use vortex_git::ConfigQuery as GitConfigQuery;

use crate::cache::{CacheError, CacheKey};
use crate::error::AppError;
use crate::extractors::{
    accept::AcceptFormat,
    path::{AppProfileLabelPath, AppProfilePath},
    query::ConfigQuery,
};
use crate::handlers::response::{ConfigResponse, PropertySourceResponse};
use crate::response::to_format;
use crate::state::AppState;

/// Handler for GET /{app}/{profile} with state.
#[instrument(skip_all, fields(app = %path.app, profile = %path.profile))]
pub async fn get_config(
    State(state): State<AppState>,
    Path(path): Path<AppProfilePath>,
    Query(_query): Query<ConfigQuery>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, AppError> {
    path.validate().map_err(AppError::BadRequest)?;

    let profiles = path.profiles();

    tracing::info!("Fetching config for {}/{:?}", path.app, profiles);

    // Use default label for this endpoint
    let label = state.config_source().default_label().to_string();

    // Get configuration (with cache if enabled)
    let response = match state.cache() {
        Some(cache) => {
            // Create cache key
            let cache_key = CacheKey::new(&path.app, profiles.join(","), &label);

            // Try to get from cache or fetch from backend
            cache
                .get_or_insert_with(cache_key, || {
                    let config_source = state.config_source();
                    let app = path.app.clone();
                    let profiles = profiles.clone();
                    async move { fetch_config(config_source, &app, profiles, &label).await }
                })
                .await
                .map_err(|e: CacheError| AppError::Internal(e.to_string()))?
        },
        None => {
            // No cache, fetch directly
            let response = fetch_config(state.config_source(), &path.app, profiles, &label).await?;
            Arc::new(response)
        },
    };

    to_format(response.as_ref(), format).map_err(|e| AppError::Internal(format!("{:?}", e)))
}

/// Handler for GET /{app}/{profile}/{label} with state.
#[instrument(skip_all, fields(
    app = %path.app,
    profile = %path.profile,
    label = %path.label
))]
pub async fn get_config_with_label(
    State(state): State<AppState>,
    Path(path): Path<AppProfileLabelPath>,
    Query(query): Query<ConfigQuery>,
    AcceptFormat(format): AcceptFormat,
) -> Result<Response, AppError> {
    path.validate().map_err(AppError::BadRequest)?;

    let profiles = path.profiles();
    let label = path.sanitized_label();

    tracing::info!(
        use_default_label = query.use_default_label,
        "Fetching config with label"
    );

    // Validate dangerous characters in label
    validate_label(&label)?;

    // Get configuration (with cache if enabled)
    let response =
        match state.cache() {
            Some(cache) => {
                // Create cache key
                let cache_key = CacheKey::new(&path.app, profiles.join(","), &label);

                // Try to get from cache or fetch from backend
                match cache
                    .get_or_insert_with(cache_key.clone(), || {
                        let config_source = state.config_source();
                        let app = path.app.clone();
                        let profiles = profiles.clone();
                        let label = label.clone();
                        async move { fetch_config(config_source, &app, profiles, &label).await }
                    })
                    .await
                {
                    Ok(response) => response,
                    Err(_) if query.use_default_label => {
                        // Fallback to default label
                        let default_label = state.config_source().default_label().to_string();
                        tracing::info!(
                            original_label = %label,
                            default_label = %default_label,
                            "Label not found, falling back to default"
                        );

                        let fallback_key =
                            CacheKey::new(&path.app, profiles.join(","), &default_label);
                        cache
                            .get_or_insert_with(fallback_key, || {
                                let config_source = state.config_source();
                                let app = path.app.clone();
                                let profiles = profiles.clone();
                                async move {
                                    fetch_config(config_source, &app, profiles, &default_label)
                                        .await
                                }
                            })
                            .await
                            .map_err(|e: CacheError| AppError::Internal(e.to_string()))?
                    },
                    Err(e) => return Err(AppError::Internal(e.to_string())),
                }
            },
            None => {
                // No cache, fetch directly with fallback logic
                let response =
                    match fetch_config(state.config_source(), &path.app, profiles.clone(), &label)
                        .await
                    {
                        Ok(response) => response,
                        Err(_) if query.use_default_label => {
                            let default_label = state.config_source().default_label();
                            tracing::info!(
                                original_label = %label,
                                default_label = %default_label,
                                "Label not found, falling back to default"
                            );
                            fetch_config(state.config_source(), &path.app, profiles, default_label)
                                .await
                                .map_err(|e| AppError::Internal(e.to_string()))?
                        },
                        Err(e) => return Err(AppError::Internal(e.to_string())),
                    };
                Arc::new(response)
            },
        };

    to_format(response.as_ref(), format).map_err(|e| AppError::Internal(format!("{:?}", e)))
}

/// Converts a ConfigValue to serde_json::Value.
fn config_value_to_json(value: &vortex_git::vortex_core::ConfigValue) -> serde_json::Value {
    use vortex_git::vortex_core::ConfigValue;

    match value {
        ConfigValue::Null => serde_json::Value::Null,
        ConfigValue::Bool(b) => serde_json::Value::Bool(*b),
        ConfigValue::Integer(i) => serde_json::Value::Number((*i).into()),
        ConfigValue::Float(f) => serde_json::Number::from_f64(f.into_inner())
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        ConfigValue::String(s) => serde_json::Value::String(s.clone()),
        ConfigValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(config_value_to_json).collect())
        },
        ConfigValue::Object(obj) => serde_json::Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), config_value_to_json(v)))
                .collect(),
        ),
    }
}

/// Validates that the label does not contain dangerous characters.
fn validate_label(label: &str) -> Result<(), AppError> {
    // Prevent path traversal
    if label.contains("..") {
        return Err(AppError::BadRequest(
            "Label cannot contain '..'".to_string(),
        ));
    }

    // Prevent control characters
    if label.chars().any(|c| c.is_control()) {
        return Err(AppError::BadRequest(
            "Label cannot contain control characters".to_string(),
        ));
    }

    Ok(())
}

/// Fetches configuration from the backend and converts it to ConfigResponse.
async fn fetch_config(
    config_source: &dyn vortex_git::ConfigSource,
    app: &str,
    profiles: Vec<String>,
    label: &str,
) -> Result<ConfigResponse, CacheError> {
    // Create query for the config source
    let git_query = GitConfigQuery::new(app, profiles.clone()).with_label_set(label);

    // Fetch from the config source
    let result = config_source
        .fetch(&git_query)
        .await
        .map_err(|e| CacheError::FetchError(e.to_string()))?;

    // Convert to response format
    Ok(ConfigResponse {
        name: result.name().to_string(),
        profiles: result.profiles().to_vec(),
        label: Some(result.label().to_string()),
        version: result.version().map(String::from),
        state: result.state().map(String::from),
        property_sources: result
            .property_sources()
            .iter()
            .map(|ps| PropertySourceResponse {
                name: ps.name.clone(),
                source: ps
                    .config
                    .as_inner()
                    .iter()
                    .map(|(k, v)| (k.clone(), config_value_to_json(v)))
                    .collect(),
            })
            .collect(),
    })
}
