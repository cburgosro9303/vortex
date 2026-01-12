//! Tests de content negotiation.
//!
//! NOTE: These tests use create_router() which only has the /health endpoint.
//! They require create_router_with_state() with a real or mock GitBackend.
//! Marking as #[ignore] until proper test infrastructure is set up.

mod helpers;

use axum::http::StatusCode;
use helpers::{assert_valid_properties, assert_valid_yaml, client};

// === JSON ===

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn returns_json_by_default() {
    let response = client().get("/myapp/dev").await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("application/json");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn returns_json_for_accept_json() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/json")
        .await;

    response.assert_content_type_contains("application/json");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn returns_json_for_accept_wildcard() {
    let response = client().get_with_accept("/myapp/dev", "*/*").await;

    response.assert_content_type_contains("application/json");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn json_response_is_valid() {
    let response = client().get("/myapp/dev").await;

    // Should not panic
    let _: serde_json::Value = response.json();
}

// === YAML ===

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn returns_yaml_for_accept_yaml() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/x-yaml")
        .await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("yaml");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn returns_yaml_for_text_yaml() {
    let response = client().get_with_accept("/myapp/dev", "text/yaml").await;

    response.assert_content_type_contains("yaml");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn yaml_response_is_valid() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/x-yaml")
        .await;

    assert_valid_yaml(&response.text());
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn yaml_contains_expected_fields() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/x-yaml")
        .await;

    let text = response.text();
    assert!(text.contains("name:"));
    assert!(text.contains("profiles:"));
    assert!(text.contains("propertySources:"));
}

// === Properties ===

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn returns_properties_for_text_plain() {
    let response = client().get_with_accept("/myapp/dev", "text/plain").await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("text/plain");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn properties_response_is_valid() {
    let response = client().get_with_accept("/myapp/dev", "text/plain").await;

    assert_valid_properties(&response.text());
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn properties_contains_comments() {
    let response = client().get_with_accept("/myapp/dev", "text/plain").await;

    let text = response.text();
    assert!(text.contains("# Application:"));
}

// === Case Insensitivity ===

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn accept_header_is_case_insensitive() {
    let response = client()
        .get_with_accept("/myapp/dev", "APPLICATION/X-YAML")
        .await;

    response.assert_content_type_contains("yaml");
}
