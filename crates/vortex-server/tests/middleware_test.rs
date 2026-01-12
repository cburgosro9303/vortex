//! Tests de middleware.

mod helpers;

use helpers::client;
use uuid::Uuid;

// === Request ID ===

#[tokio::test]
async fn response_includes_request_id() {
    let response = client().get("/health").await;

    response.assert_header_exists("x-request-id");
}

#[tokio::test]
async fn request_id_is_valid_uuid() {
    let response = client().get("/health").await;

    let id = response.header("x-request-id").unwrap();
    let parsed = Uuid::parse_str(id);

    assert!(parsed.is_ok(), "Invalid UUID: {}", id);
}

#[tokio::test]
async fn request_id_is_uuid_v7() {
    let response = client().get("/health").await;

    let id = response.header("x-request-id").unwrap();
    let parsed = Uuid::parse_str(id).unwrap();

    assert_eq!(parsed.get_version_num(), 7);
}

#[tokio::test]
async fn propagates_incoming_request_id() {
    let custom_id = "my-custom-request-id-12345";

    let response = client()
        .get_with_headers("/health", vec![("x-request-id", custom_id)])
        .await;

    response.assert_header("x-request-id", custom_id);
}

#[tokio::test]
async fn generates_different_ids_for_each_request() {
    let response1 = client().get("/health").await;
    let response2 = client().get("/health").await;

    let id1 = response1.header("x-request-id").unwrap();
    let id2 = response2.header("x-request-id").unwrap();

    assert_ne!(id1, id2);
}

// === Request ID Propagation in Different Endpoints ===

#[tokio::test]
async fn request_id_present_in_config_endpoint() {
    let response = client().get("/myapp/dev").await;

    response.assert_header_exists("x-request-id");
}

#[tokio::test]
async fn request_id_present_in_config_with_label() {
    let response = client().get("/myapp/dev/main").await;

    response.assert_header_exists("x-request-id");
}
