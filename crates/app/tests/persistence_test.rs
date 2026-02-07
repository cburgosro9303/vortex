//! Integration tests for Sprint 02 - Persistence and Collections
//!
//! These tests verify the complete flow of creating, saving, and loading
//! workspaces and collections using the file-based persistence layer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use tempfile::tempdir;

use vortex_application::ports::{CollectionRepository, WorkspaceRepository};
use vortex_domain::persistence::{
    PersistenceCollection, PersistenceFolder, PersistenceHttpMethod, SavedRequest,
};
use vortex_domain::{generate_id, generate_id_v7};
use vortex_infrastructure::{
    FileSystemCollectionRepository, FileSystemWorkspaceRepository, TokioFileSystem,
};

#[tokio::test]
async fn test_create_workspace() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let workspace_path = temp_dir.path().join("my-workspace");

    let fs = TokioFileSystem::new();
    let repo = FileSystemWorkspaceRepository::new(fs);

    // Create workspace
    let manifest = repo
        .create(&workspace_path, "My Test Workspace")
        .await
        .expect("Failed to create workspace");

    assert_eq!(manifest.name, "My Test Workspace");
    assert_eq!(manifest.schema_version, 1);

    // Verify files were created
    assert!(workspace_path.join("vortex.json").exists());
    assert!(workspace_path.join("collections").is_dir());
    assert!(workspace_path.join("environments").is_dir());
    assert!(workspace_path.join(".vortex").is_dir());
}

#[tokio::test]
async fn test_load_workspace() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let workspace_path = temp_dir.path().join("my-workspace");

    let fs = TokioFileSystem::new();
    let repo = FileSystemWorkspaceRepository::new(fs);

    // Create and then load
    let created = repo
        .create(&workspace_path, "Test Workspace")
        .await
        .expect("Failed to create workspace");

    let fs2 = TokioFileSystem::new();
    let repo2 = FileSystemWorkspaceRepository::new(fs2);
    let loaded = repo2
        .load(&workspace_path)
        .await
        .expect("Failed to load workspace");

    assert_eq!(created.name, loaded.name);
    assert_eq!(created.schema_version, loaded.schema_version);
}

#[tokio::test]
async fn test_create_collection() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let collection_path = temp_dir.path().join("my-collection");

    let fs = TokioFileSystem::new();
    let repo = FileSystemCollectionRepository::new(fs);

    let collection = PersistenceCollection::new(generate_id(), "My API");

    repo.create_collection(&collection_path, &collection)
        .await
        .expect("Failed to create collection");

    // Verify files were created
    assert!(collection_path.join("collection.json").exists());
    assert!(collection_path.join("requests").is_dir());
}

#[tokio::test]
async fn test_create_and_load_request() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let collection_path = temp_dir.path().join("test-collection");

    let fs = TokioFileSystem::new();
    let repo = FileSystemCollectionRepository::new(fs);

    // Create collection first
    let collection = PersistenceCollection::new(generate_id(), "Test Collection");
    repo.create_collection(&collection_path, &collection)
        .await
        .expect("Failed to create collection");

    // Create a request
    let request = SavedRequest::new(
        generate_id_v7(),
        "Get Users",
        PersistenceHttpMethod::Get,
        "https://api.example.com/users",
    )
    .with_header("Accept", "application/json");

    let request_path = repo
        .create_request(&collection_path, None, &request)
        .await
        .expect("Failed to create request");

    assert!(request_path.exists());
    assert!(request_path.to_string_lossy().contains("get-users.json"));

    // Load the request
    let loaded = repo
        .load_request(&request_path)
        .await
        .expect("Failed to load request");

    assert_eq!(loaded.name, "Get Users");
    assert_eq!(loaded.method, PersistenceHttpMethod::Get);
    assert_eq!(loaded.url, "https://api.example.com/users");
}

#[tokio::test]
async fn test_load_collection_tree() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let collection_path = temp_dir.path().join("full-collection");

    let fs = TokioFileSystem::new();
    let repo = FileSystemCollectionRepository::new(fs);

    // Create collection
    let collection = PersistenceCollection::new(generate_id(), "Full Collection")
        .with_description("A complete collection for testing");
    repo.create_collection(&collection_path, &collection)
        .await
        .expect("Failed to create collection");

    // Create a root-level request
    let request1 = SavedRequest::new(
        generate_id_v7(),
        "Health Check",
        PersistenceHttpMethod::Get,
        "https://api.example.com/health",
    );
    repo.create_request(&collection_path, None, &request1)
        .await
        .expect("Failed to create request 1");

    // Create a folder
    let folder = PersistenceFolder::new(generate_id(), "Users");
    let _folder_path = repo
        .create_folder(&collection_path, None, &folder)
        .await
        .expect("Failed to create folder");

    // Create a request in the folder
    let request2 = SavedRequest::new(
        generate_id_v7(),
        "Get Users",
        PersistenceHttpMethod::Get,
        "https://api.example.com/users",
    );

    // Use relative path for folder
    let relative_folder = PathBuf::from("users");
    repo.create_request(&collection_path, Some(relative_folder.as_path()), &request2)
        .await
        .expect("Failed to create request 2");

    // Load the full collection tree
    let fs2 = TokioFileSystem::new();
    let repo2 = FileSystemCollectionRepository::new(fs2);
    let tree = repo2
        .load_collection(&collection_path)
        .await
        .expect("Failed to load collection");

    assert_eq!(tree.collection.name, "Full Collection");
    assert_eq!(tree.requests.len(), 1);
    assert_eq!(tree.folders.len(), 1);
    assert_eq!(tree.folders[0].folder.name, "Users");
    assert_eq!(tree.folders[0].requests.len(), 1);
}

#[tokio::test]
async fn test_deterministic_serialization() {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let collection_path = temp_dir.path().join("deterministic-test");

    let fs = TokioFileSystem::new();
    let repo = FileSystemCollectionRepository::new(fs);

    // Create collection with specific ID for reproducibility
    let collection = PersistenceCollection::new(
        "550e8400-e29b-41d4-a716-446655440000".to_string(),
        "Deterministic Collection",
    )
    .with_variable("base_url", "https://api.example.com")
    .with_variable("api_key", "secret123");

    repo.create_collection(&collection_path, &collection)
        .await
        .expect("Failed to create collection");

    // Read the file content
    let content = std::fs::read_to_string(collection_path.join("collection.json"))
        .expect("Failed to read file");

    // Verify formatting
    assert!(content.ends_with('\n'), "Should have trailing newline");
    assert!(content.contains("  \""), "Should use 2-space indentation");

    // Verify keys are ordered alphabetically
    let base_url_pos = content
        .find("base_url")
        .expect("base_url should be in content");
    let api_key_pos = content
        .find("api_key")
        .expect("api_key should be in content");
    assert!(
        api_key_pos < base_url_pos,
        "Keys should be alphabetically ordered"
    );
}
