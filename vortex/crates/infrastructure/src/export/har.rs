//! HTTP Archive (HAR) format exporter.
//!
//! Exports requests and responses to HAR 1.2 format.

use chrono::Utc;
use serde::Serialize;
use vortex_domain::export::{ExportFormat, ExportOptions, ExportResult, ExportWarning};
use vortex_domain::request::{RequestBodyKind, RequestSpec};
use vortex_domain::response::ResponseSpec;

use super::ExportError;

/// HAR format exporter.
pub struct HarExporter;

impl HarExporter {
    /// Export requests and responses to HAR format.
    pub fn export(
        requests: &[RequestSpec],
        responses: &[ResponseSpec],
        options: &ExportOptions,
    ) -> Result<ExportResult, ExportError> {
        let mut result = ExportResult::new(String::new(), ExportFormat::Har, requests.len());

        let entries: Vec<HarEntry> = requests
            .iter()
            .enumerate()
            .map(|(i, req)| {
                let response = responses.get(i);
                Self::create_entry(req, response, options, &mut result)
            })
            .collect();

        let har = Har {
            log: HarLog {
                version: "1.2".to_string(),
                creator: HarCreator {
                    name: "Vortex".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                entries,
            },
        };

        let content = if options.pretty_print {
            serde_json::to_string_pretty(&har)
        } else {
            serde_json::to_string(&har)
        }
        .map_err(|e| ExportError::Serialization(e.to_string()))?;

        result.content = content;
        Ok(result)
    }

    fn create_entry(
        request: &RequestSpec,
        response: Option<&ResponseSpec>,
        options: &ExportOptions,
        result: &mut ExportResult,
    ) -> HarEntry {
        let started = Utc::now();

        // Build request
        let har_request = Self::build_request(request, options, result);

        // Build response
        let har_response = match response {
            Some(resp) if options.include_responses => Self::build_response(resp),
            _ => HarResponse::empty(),
        };

        // Calculate timings
        let time_ms = response.map_or(0, |r| r.duration.as_millis() as i64);

        HarEntry {
            started_date_time: started.to_rfc3339(),
            time: time_ms,
            request: har_request,
            response: har_response,
            cache: HarCache {},
            timings: HarTimings {
                send: 0,
                wait: time_ms,
                receive: 0,
            },
        }
    }

    fn build_request(
        request: &RequestSpec,
        options: &ExportOptions,
        result: &mut ExportResult,
    ) -> HarRequest {
        // Build headers
        let mut headers: Vec<HarHeader> = if options.include_headers {
            request
                .headers
                .all()
                .iter()
                .filter(|h| h.enabled)
                .map(|h| HarHeader {
                    name: h.name.clone(),
                    value: h.value.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };

        // Add auth header if present
        if options.include_auth {
            if let Some(auth_header) = Self::get_auth_header(request) {
                headers.push(auth_header);
            }
        }

        // Build query string
        let query_string: Vec<HarQueryParam> = request
            .query_params
            .all()
            .iter()
            .filter(|p| p.enabled)
            .map(|p| HarQueryParam {
                name: p.key.clone(),
                value: p.value.clone(),
            })
            .collect();

        // Build post data
        let post_data = if options.include_body {
            Self::build_post_data(request, result)
        } else {
            None
        };

        // Build URL with query params
        let url = if query_string.is_empty() {
            request.url.clone()
        } else {
            let params: Vec<String> = query_string
                .iter()
                .map(|p| format!("{}={}", p.name, p.value))
                .collect();
            format!("{}?{}", request.url, params.join("&"))
        };

        let body_size = if request.body.is_empty() {
            -1
        } else {
            request.body.content.len() as i64
        };

        HarRequest {
            method: request.method.as_str().to_uppercase(),
            url,
            http_version: "HTTP/1.1".to_string(),
            headers,
            query_string,
            post_data,
            headers_size: -1,
            body_size,
        }
    }

    fn build_response(response: &ResponseSpec) -> HarResponse {
        let headers: Vec<HarHeader> = response
            .headers_map
            .iter()
            .map(|(name, value)| HarHeader {
                name: name.clone(),
                value: value.clone(),
            })
            .collect();

        let content = HarContent {
            size: response.size as i64,
            mime_type: response.content_type.clone().unwrap_or_default(),
            text: Some(response.body.clone()),
        };

        HarResponse {
            status: response.status as i32,
            status_text: response.status_text.clone(),
            http_version: "HTTP/1.1".to_string(),
            headers,
            content,
            redirect_url: String::new(),
            headers_size: -1,
            body_size: response.size as i64,
        }
    }

    fn build_post_data(request: &RequestSpec, result: &mut ExportResult) -> Option<HarPostData> {
        match &request.body.kind {
            RequestBodyKind::None => None,
            RequestBodyKind::Raw { content_type } => Some(HarPostData {
                mime_type: content_type.clone(),
                text: Some(request.body.content.clone()),
                params: Vec::new(),
            }),
            RequestBodyKind::FormUrlEncoded => {
                // Parse the content as URL encoded params
                let params: Vec<HarParam> = request
                    .body
                    .content
                    .split('&')
                    .filter_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        let name = parts.next()?.to_string();
                        let value = parts.next().unwrap_or("").to_string();
                        Some(HarParam {
                            name,
                            value: Some(value),
                            file_name: None,
                            content_type: None,
                        })
                    })
                    .collect();

                Some(HarPostData {
                    mime_type: "application/x-www-form-urlencoded".to_string(),
                    text: Some(request.body.content.clone()),
                    params,
                })
            }
            RequestBodyKind::FormData => {
                result.add_warning(
                    ExportWarning::new("Multipart form data may not export correctly")
                        .with_source(&request.url),
                );

                Some(HarPostData {
                    mime_type: "multipart/form-data".to_string(),
                    text: Some(request.body.content.clone()),
                    params: Vec::new(),
                })
            }
        }
    }

    fn get_auth_header(request: &RequestSpec) -> Option<HarHeader> {
        use vortex_domain::auth::AuthConfig;

        match &request.auth {
            AuthConfig::None => None,
            AuthConfig::Basic { username, password } => {
                let credentials = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("{}:{}", username, password),
                );
                Some(HarHeader {
                    name: "Authorization".to_string(),
                    value: format!("Basic {}", credentials),
                })
            }
            AuthConfig::Bearer { token, prefix } => Some(HarHeader {
                name: "Authorization".to_string(),
                value: format!("{} {}", prefix, token),
            }),
            AuthConfig::ApiKey { key, name, location } => {
                use vortex_domain::auth::ApiKeyLocation;
                match location {
                    ApiKeyLocation::Header => Some(HarHeader {
                        name: name.clone(),
                        value: key.clone(),
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

// HAR format structs

#[derive(Serialize)]
struct Har {
    log: HarLog,
}

#[derive(Serialize)]
struct HarLog {
    version: String,
    creator: HarCreator,
    entries: Vec<HarEntry>,
}

#[derive(Serialize)]
struct HarCreator {
    name: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarEntry {
    started_date_time: String,
    time: i64,
    request: HarRequest,
    response: HarResponse,
    cache: HarCache,
    timings: HarTimings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarRequest {
    method: String,
    url: String,
    http_version: String,
    headers: Vec<HarHeader>,
    query_string: Vec<HarQueryParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post_data: Option<HarPostData>,
    headers_size: i64,
    body_size: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarResponse {
    status: i32,
    status_text: String,
    http_version: String,
    headers: Vec<HarHeader>,
    content: HarContent,
    redirect_url: String,
    headers_size: i64,
    body_size: i64,
}

impl HarResponse {
    fn empty() -> Self {
        Self {
            status: 0,
            status_text: String::new(),
            http_version: "HTTP/1.1".to_string(),
            headers: Vec::new(),
            content: HarContent {
                size: 0,
                mime_type: String::new(),
                text: None,
            },
            redirect_url: String::new(),
            headers_size: -1,
            body_size: -1,
        }
    }
}

#[derive(Serialize)]
struct HarHeader {
    name: String,
    value: String,
}

#[derive(Serialize)]
struct HarQueryParam {
    name: String,
    value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarPostData {
    mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    params: Vec<HarParam>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarParam {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarContent {
    size: i64,
    mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Serialize)]
struct HarCache {}

#[derive(Serialize)]
struct HarTimings {
    send: i64,
    wait: i64,
    receive: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_simple_request() {
        let request = RequestSpec::get("https://api.example.com/users");
        let options = ExportOptions::new(ExportFormat::Har);

        let result = HarExporter::export(&[request], &[], &options).unwrap();

        assert_eq!(result.format, ExportFormat::Har);
        assert_eq!(result.request_count, 1);
        assert!(result.content.contains("api.example.com"));
        assert!(result.content.contains("GET"));
    }

    #[test]
    fn test_export_with_response() {
        use std::collections::HashMap;
        use std::time::Duration;

        let request = RequestSpec::get("https://api.example.com/users");
        let response = ResponseSpec::new(
            200u16,
            HashMap::new(),
            b"Hello".to_vec(),
            Duration::from_millis(100),
        );

        let mut options = ExportOptions::new(ExportFormat::Har);
        options.include_responses = true;

        let result = HarExporter::export(&[request], &[response], &options).unwrap();

        assert!(result.content.contains("200"));
    }
}
