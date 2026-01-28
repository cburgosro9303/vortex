//! Postman to Vortex Mapping Logic
//!
//! This module converts Postman Collection and Environment types to Vortex native format.

#![allow(missing_docs)]

use super::environment_types::PostmanEnvironment;
use super::types::{
    PostmanAuth, PostmanBody, PostmanCollection, PostmanHeader, PostmanItem, PostmanQueryParam,
    PostmanVariable,
};
use super::warning::{ImportWarning, WarningSeverity};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Result of mapping a collection
#[derive(Debug)]
pub struct MappedCollection {
    pub name: String,
    pub description: Option<String>,
    pub items: Vec<MappedItem>,
    pub variables: Vec<MappedVariable>,
    pub warnings: Vec<ImportWarning>,
}

/// A mapped item (folder or request)
#[derive(Debug)]
pub enum MappedItem {
    Folder(MappedFolder),
    Request(MappedRequest),
}

/// A mapped folder
#[derive(Debug)]
pub struct MappedFolder {
    pub name: String,
    pub description: Option<String>,
    pub items: Vec<MappedItem>,
}

/// A mapped request in Vortex format
#[derive(Debug)]
pub struct MappedRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub query_params: BTreeMap<String, String>,
    pub body: Option<MappedBody>,
    pub auth: Option<MappedAuth>,
}

/// Mapped body
#[derive(Debug, Clone)]
pub enum MappedBody {
    Json(Value),
    Text(String),
    FormUrlEncoded(Vec<(String, String)>),
    FormData(Vec<FormDataField>),
    Binary { filename: Option<String> },
    GraphQL { query: String, variables: Option<String> },
}

/// Form data field
#[derive(Debug, Clone)]
pub struct FormDataField {
    pub key: String,
    pub value: FormDataValue,
}

#[derive(Debug, Clone)]
pub enum FormDataValue {
    Text(String),
    File { src: Option<String> },
}

/// Mapped auth
#[derive(Debug, Clone)]
pub enum MappedAuth {
    None,
    Bearer { token: String },
    Basic { username: String, password: String },
    ApiKey { key: String, value: String, in_header: bool },
    OAuth2 { access_token: Option<String>, token_url: Option<String> },
}

/// Mapped variable
#[derive(Debug)]
pub struct MappedVariable {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    pub is_secret: bool,
}

/// Mapped environment
#[derive(Debug)]
pub struct MappedEnvironment {
    pub name: String,
    pub variables: Vec<MappedVariable>,
    pub warnings: Vec<ImportWarning>,
}

/// Map HTTP method to Vortex format
pub fn map_http_method(method: &str) -> String {
    method.to_uppercase()
}

/// Map headers from Postman format
pub fn map_headers(headers: &[PostmanHeader]) -> (BTreeMap<String, String>, Vec<ImportWarning>) {
    let mut warnings = Vec::new();
    let mapped: BTreeMap<String, String> = headers
        .iter()
        .filter(|h| !h.disabled)
        .map(|h| (h.key.clone(), h.value.clone()))
        .collect();

    let disabled_count = headers.iter().filter(|h| h.disabled).count();
    if disabled_count > 0 {
        warnings.push(ImportWarning::new(
            "headers",
            format!("{} disabled header(s) were skipped", disabled_count),
            WarningSeverity::Info,
        ));
    }

    (mapped, warnings)
}

/// Map query parameters
pub fn map_query_params(
    params: &[PostmanQueryParam],
) -> (BTreeMap<String, String>, Vec<ImportWarning>) {
    let mut warnings = Vec::new();
    let mapped: BTreeMap<String, String> = params
        .iter()
        .filter(|p| !p.disabled)
        .map(|p| (p.key.clone(), p.value.clone().unwrap_or_default()))
        .collect();

    let disabled_count = params.iter().filter(|p| p.disabled).count();
    if disabled_count > 0 {
        warnings.push(ImportWarning::new(
            "query_params",
            format!("{} disabled query param(s) were skipped", disabled_count),
            WarningSeverity::Info,
        ));
    }

    (mapped, warnings)
}

/// Map body from Postman format
pub fn map_body(body: &Option<PostmanBody>) -> (Option<MappedBody>, Vec<ImportWarning>) {
    let mut warnings = Vec::new();

    let mapped = body.as_ref().and_then(|b| {
        match b.mode.as_str() {
            "raw" => {
                let raw = b.raw.clone().unwrap_or_default();
                if raw.is_empty() {
                    return None;
                }

                // Check if it's JSON
                let language = b
                    .options
                    .as_ref()
                    .and_then(|o| o.raw.as_ref())
                    .and_then(|r| r.language.clone())
                    .unwrap_or_default();

                if language == "json" || raw.trim_start().starts_with('{') || raw.trim_start().starts_with('[') {
                    if let Ok(json_value) = serde_json::from_str::<Value>(&raw) {
                        return Some(MappedBody::Json(json_value));
                    }
                }
                Some(MappedBody::Text(raw))
            }
            "urlencoded" => {
                let params: Vec<(String, String)> = b
                    .urlencoded
                    .iter()
                    .filter(|p| !p.disabled)
                    .map(|p| (p.key.clone(), p.value.clone().unwrap_or_default()))
                    .collect();

                if params.is_empty() {
                    return None;
                }
                Some(MappedBody::FormUrlEncoded(params))
            }
            "formdata" => {
                let fields: Vec<FormDataField> = b
                    .formdata
                    .iter()
                    .filter(|p| !p.disabled)
                    .map(|p| {
                        let value = if p.param_type.as_deref() == Some("file") {
                            FormDataValue::File { src: p.src.clone() }
                        } else {
                            FormDataValue::Text(p.value.clone().unwrap_or_default())
                        };
                        FormDataField { key: p.key.clone(), value }
                    })
                    .collect();

                if fields.is_empty() {
                    return None;
                }

                // Warn about file uploads
                let file_count = fields.iter().filter(|f| matches!(f.value, FormDataValue::File { .. })).count();
                if file_count > 0 {
                    warnings.push(ImportWarning::new(
                        "body.formdata",
                        format!("{} file field(s) were imported without actual file content", file_count),
                        WarningSeverity::Warning,
                    ));
                }

                Some(MappedBody::FormData(fields))
            }
            "file" => {
                let filename = b.file.as_ref().and_then(|f| f.src.clone());
                warnings.push(ImportWarning::new(
                    "body.file",
                    "Binary file body imported without actual content. You'll need to re-attach the file.".to_string(),
                    WarningSeverity::Warning,
                ));
                Some(MappedBody::Binary { filename })
            }
            "graphql" => {
                if let Some(ref gql) = b.graphql {
                    Some(MappedBody::GraphQL {
                        query: gql.query.clone(),
                        variables: gql.variables.clone(),
                    })
                } else {
                    None
                }
            }
            other => {
                warnings.push(ImportWarning::new(
                    "body",
                    format!("Unknown body mode '{}' was skipped", other),
                    WarningSeverity::Warning,
                ));
                None
            }
        }
    });

    (mapped, warnings)
}

/// Map authentication
pub fn map_auth(auth: &Option<PostmanAuth>) -> (Option<MappedAuth>, Vec<ImportWarning>) {
    let mut warnings = Vec::new();

    let mapped = auth.as_ref().and_then(|a| {
        match a.auth_type.as_str() {
            "noauth" => Some(MappedAuth::None),
            "bearer" => {
                let token = a.get_param(&a.bearer, "token").unwrap_or_default();
                Some(MappedAuth::Bearer { token })
            }
            "basic" => {
                let username = a.get_param(&a.basic, "username").unwrap_or_default();
                let password = a.get_param(&a.basic, "password").unwrap_or_default();
                Some(MappedAuth::Basic { username, password })
            }
            "apikey" => {
                let key = a.get_param(&a.apikey, "key").unwrap_or_default();
                let value = a.get_param(&a.apikey, "value").unwrap_or_default();
                let in_location = a.get_param(&a.apikey, "in").unwrap_or_else(|| "header".to_string());
                Some(MappedAuth::ApiKey {
                    key,
                    value,
                    in_header: in_location == "header",
                })
            }
            "oauth2" => {
                let access_token = a.get_param(&a.oauth2, "accessToken");
                let token_url = a.get_param(&a.oauth2, "accessTokenUrl");

                warnings.push(ImportWarning::new(
                    "auth.oauth2",
                    "OAuth2 configuration imported partially. You may need to re-configure tokens.".to_string(),
                    WarningSeverity::Warning,
                ));

                Some(MappedAuth::OAuth2 { access_token, token_url })
            }
            "digest" | "hawk" | "ntlm" | "awsv4" => {
                warnings.push(ImportWarning::new(
                    "auth",
                    format!("Authentication type '{}' is not supported and was skipped", a.auth_type),
                    WarningSeverity::Warning,
                ));
                None
            }
            other => {
                warnings.push(ImportWarning::new(
                    "auth",
                    format!("Unknown authentication type '{}' was skipped", other),
                    WarningSeverity::Warning,
                ));
                None
            }
        }
    });

    (mapped, warnings)
}

/// Map collection variables
pub fn map_collection_variables(variables: &[PostmanVariable]) -> Vec<MappedVariable> {
    variables
        .iter()
        .filter(|v| !v.disabled)
        .map(|v| MappedVariable {
            name: v.key.clone(),
            value: v.value.clone().unwrap_or_default(),
            enabled: true,
            is_secret: v.var_type.as_deref() == Some("secret"),
        })
        .collect()
}

/// Map a single Postman item (recursively handles folders)
pub fn map_postman_item(
    item: &PostmanItem,
    path: &str,
    depth: usize,
    max_depth: usize,
) -> (Option<MappedItem>, Vec<ImportWarning>) {
    let mut warnings = Vec::new();
    let current_path = if path.is_empty() {
        item.name.clone()
    } else {
        format!("{}/{}", path, item.name)
    };

    if item.is_folder() {
        // Check depth limit
        if depth >= max_depth {
            warnings.push(ImportWarning::new(
                &current_path,
                format!("Folder exceeds maximum depth of {} and was flattened", max_depth),
                WarningSeverity::Warning,
            ));
        }

        // Check for scripts
        if !item.event.is_empty() {
            warnings.push(ImportWarning::new(
                &current_path,
                "Folder-level scripts (pre-request/test) are not supported and were skipped".to_string(),
                WarningSeverity::Info,
            ));
        }

        let mut folder_items = Vec::new();
        if let Some(ref sub_items) = item.item {
            for sub_item in sub_items {
                let (mapped, item_warnings) =
                    map_postman_item(sub_item, &current_path, depth + 1, max_depth);
                warnings.extend(item_warnings);
                if let Some(m) = mapped {
                    folder_items.push(m);
                }
            }
        }

        return (
            Some(MappedItem::Folder(MappedFolder {
                name: item.name.clone(),
                description: item.description.clone(),
                items: folder_items,
            })),
            warnings,
        );
    }

    // It's a request
    if let Some(ref request) = item.request {
        // Check for scripts
        if !item.event.is_empty() {
            warnings.push(ImportWarning::new(
                &current_path,
                "Request scripts (pre-request/test) are not supported and were skipped".to_string(),
                WarningSeverity::Info,
            ));
        }

        let (headers, header_warnings) = map_headers(&request.header);
        warnings.extend(header_warnings);

        let query_params = request.url.query_params();
        let (params, param_warnings) = map_query_params(&query_params);
        warnings.extend(param_warnings);

        let (body, body_warnings) = map_body(&request.body);
        warnings.extend(body_warnings);

        let (auth, auth_warnings) = map_auth(&request.auth);
        warnings.extend(auth_warnings);

        return (
            Some(MappedItem::Request(MappedRequest {
                id: uuid::Uuid::new_v4().to_string(),
                name: item.name.clone(),
                description: item.description.clone().or(request.description.clone()),
                method: map_http_method(&request.method),
                url: request.url.raw(),
                headers,
                query_params: params,
                body,
                auth,
            })),
            warnings,
        );
    }

    // Item without request or sub-items - skip it
    warnings.push(ImportWarning::new(
        &current_path,
        "Item has no request or sub-items and was skipped".to_string(),
        WarningSeverity::Info,
    ));

    (None, warnings)
}

/// Map a complete Postman collection
pub fn map_postman_collection(
    collection: &PostmanCollection,
    max_depth: usize,
) -> MappedCollection {
    let mut warnings = Vec::new();
    let mut items = Vec::new();

    for item in &collection.item {
        let (mapped, item_warnings) = map_postman_item(item, "", 0, max_depth);
        warnings.extend(item_warnings);
        if let Some(m) = mapped {
            items.push(m);
        }
    }

    // Check for collection-level auth
    if let Some(ref auth) = collection.auth {
        let (_, auth_warnings) = map_auth(&Some(auth.clone()));
        if !auth_warnings.is_empty() {
            warnings.extend(auth_warnings);
        }
    }

    // Check for collection-level events
    if !collection.event.is_empty() {
        warnings.push(ImportWarning::new(
            "collection",
            "Collection-level scripts (pre-request/test) are not supported and were skipped"
                .to_string(),
            WarningSeverity::Info,
        ));
    }

    let variables = map_collection_variables(&collection.variable);

    MappedCollection {
        name: collection.info.name.clone(),
        description: collection.info.description.clone(),
        items,
        variables,
        warnings,
    }
}

/// Map a Postman environment to Vortex format
pub fn map_postman_environment(env: &PostmanEnvironment) -> MappedEnvironment {
    let warnings = Vec::new();

    let variables = env
        .values
        .iter()
        .map(|v| MappedVariable {
            name: v.key.clone(),
            value: v.value.clone(),
            enabled: v.enabled,
            is_secret: v.is_secret(),
        })
        .collect();

    MappedEnvironment {
        name: env.name.clone(),
        variables,
        warnings,
    }
}

/// Convert MappedBody to Vortex JSON format
pub fn body_to_vortex_json(body: &MappedBody) -> Value {
    match body {
        MappedBody::Json(v) => json!({
            "type": "json",
            "content": v
        }),
        MappedBody::Text(s) => json!({
            "type": "text",
            "content": s
        }),
        MappedBody::FormUrlEncoded(params) => {
            let obj: Map<String, Value> = params
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            json!({
                "type": "form_urlencoded",
                "content": obj
            })
        }
        MappedBody::FormData(fields) => {
            let arr: Vec<Value> = fields
                .iter()
                .map(|f| {
                    match &f.value {
                        FormDataValue::Text(s) => json!({
                            "key": f.key,
                            "type": "text",
                            "value": s
                        }),
                        FormDataValue::File { src } => json!({
                            "key": f.key,
                            "type": "file",
                            "src": src
                        }),
                    }
                })
                .collect();
            json!({
                "type": "form_data",
                "content": arr
            })
        }
        MappedBody::Binary { filename } => json!({
            "type": "binary",
            "filename": filename
        }),
        MappedBody::GraphQL { query, variables } => {
            let mut obj = json!({
                "type": "graphql",
                "query": query
            });
            if let Some(vars) = variables {
                obj["variables"] = json!(vars);
            }
            obj
        }
    }
}

/// Convert MappedAuth to Vortex JSON format
pub fn auth_to_vortex_json(auth: &MappedAuth) -> Value {
    match auth {
        MappedAuth::None => json!({ "type": "none" }),
        MappedAuth::Bearer { token } => json!({
            "type": "bearer",
            "token": token
        }),
        MappedAuth::Basic { username, password } => json!({
            "type": "basic",
            "username": username,
            "password": password
        }),
        MappedAuth::ApiKey { key, value, in_header } => json!({
            "type": "apikey",
            "key": key,
            "value": value,
            "in": if *in_header { "header" } else { "query" }
        }),
        MappedAuth::OAuth2 { access_token, token_url } => json!({
            "type": "oauth2",
            "access_token": access_token,
            "token_url": token_url
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_method() {
        assert_eq!(map_http_method("get"), "GET");
        assert_eq!(map_http_method("POST"), "POST");
        assert_eq!(map_http_method("pAtCh"), "PATCH");
    }

    #[test]
    fn test_map_headers_filters_disabled() {
        let headers = vec![
            PostmanHeader {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
                description: None,
                header_type: None,
                disabled: false,
            },
            PostmanHeader {
                key: "X-Debug".to_string(),
                value: "true".to_string(),
                description: None,
                header_type: None,
                disabled: true,
            },
        ];

        let (mapped, warnings) = map_headers(&headers);
        assert_eq!(mapped.len(), 1);
        assert!(mapped.contains_key("Content-Type"));
        assert!(!mapped.contains_key("X-Debug"));
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_map_body_json() {
        let body = PostmanBody {
            mode: "raw".to_string(),
            raw: Some(r#"{"name": "test"}"#.to_string()),
            urlencoded: vec![],
            formdata: vec![],
            file: None,
            graphql: None,
            options: None,
        };

        let (mapped, _) = map_body(&Some(body));
        assert!(matches!(mapped, Some(MappedBody::Json(_))));
    }

    #[test]
    fn test_map_auth_bearer() {
        use super::super::types::PostmanAuthParam;

        let auth = PostmanAuth {
            auth_type: "bearer".to_string(),
            noauth: None,
            basic: vec![],
            bearer: vec![PostmanAuthParam {
                key: "token".to_string(),
                value: Some("abc123".to_string()),
                param_type: None,
            }],
            apikey: vec![],
            oauth2: vec![],
            digest: vec![],
            hawk: vec![],
            ntlm: vec![],
            awsv4: vec![],
        };

        let (mapped, warnings) = map_auth(&Some(auth));
        assert!(warnings.is_empty());
        match mapped {
            Some(MappedAuth::Bearer { token }) => assert_eq!(token, "abc123"),
            _ => panic!("Expected Bearer auth"),
        }
    }
}
