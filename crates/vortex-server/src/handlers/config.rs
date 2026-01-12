//! Configuration endpoint handlers.

use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use tracing::instrument;
use vortex_git::ConfigQuery as GitConfigQuery;

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

    // Create query for the config source
    let git_query = GitConfigQuery::new(&path.app, profiles.clone());

    // Fetch from the config source
    let result = state
        .config_source()
        .fetch(&git_query)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Convert to response format
    let response = ConfigResponse {
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
    };

    to_format(&response, format).map_err(|e| AppError::Internal(format!("{:?}", e)))
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

    // Create query for the config source with label
    let git_query = GitConfigQuery::new(&path.app, profiles.clone()).with_label_set(&label);

    // Fetch from the config source
    let result = state
        .config_source()
        .fetch(&git_query)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Convert to response format
    let response = ConfigResponse {
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
    };

    to_format(&response, format).map_err(|e| AppError::Internal(format!("{:?}", e)))
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
