use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use vortex_server::create_router;

// NOTE: These tests use create_router() which only has the /health endpoint.
// They require create_router_with_state() with a real or mock GitBackend.
// Marking as #[ignore] until proper test infrastructure is set up.

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn get_config_returns_200_for_valid_path() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn get_config_returns_correct_app_name() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/payment-service/production")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(config["name"], "payment-service");
    assert_eq!(config["profiles"][0], "production");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn get_config_supports_multiple_profiles() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev,local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let profiles = config["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0], "dev");
    assert_eq!(profiles[1], "local");
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn get_config_returns_json_content_type() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("application/json"));
}

#[tokio::test]
#[ignore = "requires GitBackend - create_router() only has /health endpoint"]
async fn get_config_has_property_sources() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let sources = config["propertySources"].as_array().unwrap();
    assert!(!sources.is_empty());
}
