//! `OpenAPI` 3.0 format exporter.
//!
//! Exports requests to `OpenAPI` 3.0 specification.

use std::collections::BTreeMap;

use serde::Serialize;
use vortex_domain::export::{ExportFormat, ExportOptions, ExportResult, ExportWarning};
use vortex_domain::request::{HttpMethod, RequestBodyKind, RequestSpec};

use super::ExportError;

/// `OpenAPI` 3.0 exporter.
pub struct OpenApiExporter;

impl OpenApiExporter {
    /// Export requests to `OpenAPI` 3.0 format.
    #[allow(clippy::missing_errors_doc)]
    pub fn export(
        requests: &[RequestSpec],
        options: &ExportOptions,
    ) -> Result<ExportResult, ExportError> {
        let mut result = ExportResult::new(String::new(), ExportFormat::OpenApi3, requests.len());

        // Group requests by path
        let mut paths: BTreeMap<String, PathItem> = BTreeMap::new();

        for request in requests {
            if let Some((path, _base_url)) = Self::extract_path(&request.url) {
                let operation = Self::create_operation(request, options, &mut result);
                let method = request.method.as_str().to_lowercase();

                let path_item = paths.entry(path).or_default();
                match method.as_str() {
                    "get" => path_item.get = Some(operation),
                    "post" => path_item.post = Some(operation),
                    "put" => path_item.put = Some(operation),
                    "delete" => path_item.delete = Some(operation),
                    "patch" => path_item.patch = Some(operation),
                    "head" => path_item.head = Some(operation),
                    "options" => path_item.options = Some(operation),
                    _ => {
                        result.add_warning(
                            ExportWarning::new(format!("Unsupported method: {method}"))
                                .with_source(&request.url),
                        );
                    }
                }
            } else {
                result.add_warning(
                    ExportWarning::new("Could not parse URL").with_source(&request.url),
                );
            }
        }

        // Build OpenAPI spec
        let spec = OpenApiSpec {
            openapi: "3.0.0".to_string(),
            info: Info {
                title: options
                    .api_title
                    .clone()
                    .unwrap_or_else(|| "API".to_string()),
                version: options
                    .api_version
                    .clone()
                    .unwrap_or_else(|| "1.0.0".to_string()),
                description: options.api_description.clone(),
            },
            servers: Self::extract_servers(requests),
            paths,
        };

        // Serialize to YAML
        let content =
            serde_yaml::to_string(&spec).map_err(|e| ExportError::Serialization(e.to_string()))?;

        result.content = content;
        Ok(result)
    }

    fn extract_path(url: &str) -> Option<(String, String)> {
        // Remove protocol
        let url = url
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        // Find the path start
        let path_start = url.find('/')?;
        let base_url = &url[..path_start];
        let path = &url[path_start..];

        // Remove query string
        let path = path.split('?').next()?;

        Some((path.to_string(), format!("https://{base_url}")))
    }

    fn extract_servers(requests: &[RequestSpec]) -> Vec<Server> {
        let mut servers: Vec<String> = Vec::new();

        for request in requests {
            let url = &request.url;
            if let Some(idx) = url.find("://") {
                let rest = &url[idx + 3..];
                if let Some(path_idx) = rest.find('/') {
                    let server = format!("{}{}", &url[..idx + 3], &rest[..path_idx]);
                    if !servers.contains(&server) {
                        servers.push(server);
                    }
                }
            }
        }

        servers.into_iter().map(|url| Server { url }).collect()
    }

    fn create_operation(
        request: &RequestSpec,
        options: &ExportOptions,
        _result: &mut ExportResult,
    ) -> Operation {
        let mut parameters = Vec::new();

        // Add query parameters
        for param in request.query_params.all() {
            if param.enabled {
                parameters.push(Parameter {
                    name: param.key.clone(),
                    location: "query".to_string(),
                    required: false,
                    schema: Schema {
                        schema_type: "string".to_string(),
                        example: Some(param.value.clone()),
                    },
                });
            }
        }

        // Add path parameters (detected from {param} in URL)
        let path_params = Self::extract_path_params(&request.url);
        for param in path_params {
            parameters.push(Parameter {
                name: param,
                location: "path".to_string(),
                required: true,
                schema: Schema {
                    schema_type: "string".to_string(),
                    example: None,
                },
            });
        }

        // Build request body
        let request_body = if options.include_body && Self::method_has_body(&request.method) {
            Self::create_request_body(request)
        } else {
            None
        };

        // Generate operation ID
        let operation_id = Self::generate_operation_id(request);

        Operation {
            operation_id: Some(operation_id),
            summary: None,
            description: request.description.clone(),
            parameters: if parameters.is_empty() {
                None
            } else {
                Some(parameters)
            },
            request_body,
            responses: Self::default_responses(),
        }
    }

    fn extract_path_params(url: &str) -> Vec<String> {
        let mut params = Vec::new();
        let mut in_param = false;
        let mut current = String::new();

        for ch in url.chars() {
            match ch {
                '{' => {
                    in_param = true;
                    current.clear();
                }
                '}' => {
                    if in_param && !current.is_empty() {
                        params.push(current.clone());
                    }
                    in_param = false;
                }
                _ if in_param => {
                    current.push(ch);
                }
                _ => {}
            }
        }

        params
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    const fn method_has_body(method: &HttpMethod) -> bool {
        matches!(
            method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
        )
    }

    fn create_request_body(request: &RequestSpec) -> Option<RequestBody> {
        let (media_type, example) = match &request.body.kind {
            RequestBodyKind::None => return None,
            RequestBodyKind::Raw { content_type } => {
                (content_type.clone(), Some(request.body.content.clone()))
            }
            RequestBodyKind::FormUrlEncoded => (
                "application/x-www-form-urlencoded".to_string(),
                Some(request.body.content.clone()),
            ),
            RequestBodyKind::FormData => ("multipart/form-data".to_string(), None),
        };

        let mut content = BTreeMap::new();
        content.insert(
            media_type,
            MediaType {
                schema: Schema {
                    schema_type: "object".to_string(),
                    example,
                },
            },
        );

        Some(RequestBody {
            required: true,
            content,
        })
    }

    fn generate_operation_id(request: &RequestSpec) -> String {
        // Extract last path segment and combine with method
        let path = request.url.split('/').next_back().unwrap_or("resource");
        let path = path.split('?').next().unwrap_or(path);
        let path = path.trim_matches(|c: char| !c.is_alphanumeric());

        let method = request.method.as_str().to_lowercase();

        if path.is_empty() {
            method
        } else {
            format!("{}{}", method, capitalize(path))
        }
    }

    fn default_responses() -> BTreeMap<String, Response> {
        let mut responses = BTreeMap::new();
        responses.insert(
            "200".to_string(),
            Response {
                description: "Successful response".to_string(),
            },
        );
        responses.insert(
            "400".to_string(),
            Response {
                description: "Bad request".to_string(),
            },
        );
        responses.insert(
            "500".to_string(),
            Response {
                description: "Internal server error".to_string(),
            },
        );
        responses
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
}

// OpenAPI structs

#[derive(Serialize)]
struct OpenApiSpec {
    openapi: String,
    info: Info,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    servers: Vec<Server>,
    paths: BTreeMap<String, PathItem>,
}

#[derive(Serialize)]
struct Info {
    title: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
struct Server {
    url: String,
}

#[derive(Serialize, Default)]
struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delete: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    head: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Operation>,
}

#[allow(clippy::struct_field_names)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Operation {
    #[serde(skip_serializing_if = "Option::is_none")]
    operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<Parameter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_body: Option<RequestBody>,
    responses: BTreeMap<String, Response>,
}

#[derive(Serialize)]
struct Parameter {
    name: String,
    #[serde(rename = "in")]
    location: String,
    required: bool,
    schema: Schema,
}

#[derive(Serialize)]
struct Schema {
    #[serde(rename = "type")]
    schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    example: Option<String>,
}

#[derive(Serialize)]
struct RequestBody {
    required: bool,
    content: BTreeMap<String, MediaType>,
}

#[derive(Serialize)]
struct MediaType {
    schema: Schema,
}

#[derive(Serialize)]
struct Response {
    description: String,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_export_simple_request() {
        let request = RequestSpec::get("https://api.example.com/users");
        let options = ExportOptions::new(ExportFormat::OpenApi3);

        let result = OpenApiExporter::export(&[request], &options).unwrap();

        assert_eq!(result.format, ExportFormat::OpenApi3);
        assert!(result.content.contains("openapi: 3.0.0"));
        assert!(result.content.contains("/users"));
        assert!(result.content.contains("get:"));
    }

    #[test]
    fn test_export_with_path_params() {
        let mut request = RequestSpec::get("https://api.example.com/users/{id}");
        request.url = "https://api.example.com/users/{id}".to_string();
        let options = ExportOptions::new(ExportFormat::OpenApi3);

        let result = OpenApiExporter::export(&[request], &options).unwrap();

        assert!(result.content.contains("name: id"));
        assert!(result.content.contains("in: path"));
    }

    #[test]
    fn test_export_with_api_info() {
        let request = RequestSpec::get("https://api.example.com/users");
        let options = ExportOptions::new(ExportFormat::OpenApi3).with_api_info(
            "My API",
            "2.0.0",
            Some("API Description".to_string()),
        );

        let result = OpenApiExporter::export(&[request], &options).unwrap();

        assert!(result.content.contains("title: My API"));
        assert!(result.content.contains("version: 2.0.0"));
        assert!(result.content.contains("description: API Description"));
    }

    #[test]
    fn test_extract_path_params() {
        let params = OpenApiExporter::extract_path_params("/users/{id}/posts/{postId}");
        assert_eq!(params, vec!["id", "postId"]);
    }
}
