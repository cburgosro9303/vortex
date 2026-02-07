//! Postman Importer - Main Import Logic
//!
//! This module provides the main import functionality for Postman collections
//! and environments, including validation, preview, and full import.

use super::environment_types::PostmanEnvironment;
use super::mapper::{
    MappedItem, auth_to_vortex_json, body_to_vortex_json, map_postman_collection,
    map_postman_environment,
};
use super::types::PostmanCollection;
use super::warning::{ImportWarning, WarningStats};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Import configuration options
#[derive(Debug, Clone)]
pub struct ImportConfig {
    /// Maximum file size in bytes (default: 10MB)
    pub max_file_size: usize,
    /// Maximum folder nesting depth (default: 10)
    pub max_depth: usize,
    /// Maximum number of items (requests + folders) (default: 1000)
    pub max_items: usize,
    /// Whether to skip items with errors (default: true)
    pub skip_on_error: bool,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_depth: 10,
            max_items: 1000,
            skip_on_error: true,
        }
    }
}

/// Import error types
#[derive(Debug, Error)]
pub enum ImportError {
    /// File was not found at the specified path
    #[error("File not found: {0}")]
    FileNotFound(String),
    /// File exceeds the maximum allowed size
    #[error("File too large: {size} bytes exceeds maximum of {max} bytes")]
    FileTooLarge {
        /// Actual file size in bytes
        size: usize,
        /// Maximum allowed size in bytes
        max: usize,
    },
    /// JSON parsing failed
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    /// File is not a valid Postman format
    #[error("Invalid Postman format: {0}")]
    InvalidFormat(String),
    /// IO operation failed
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// Too many items in the collection
    #[error("Too many items: {count} exceeds maximum of {max}")]
    TooManyItems {
        /// Actual item count
        count: usize,
        /// Maximum allowed items
        max: usize,
    },
    /// Import was aborted due to errors
    #[error("Import aborted due to errors")]
    Aborted,
}

/// Result of validating a file before import
#[derive(Debug)]
pub struct ValidationResult {
    /// Whether the file is valid for import
    pub is_valid: bool,
    /// Detected format of the file
    pub format: ImportFormat,
    /// List of validation issues found
    pub issues: Vec<String>,
}

/// Detected import format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    /// Postman Collection v2.1 format
    PostmanCollectionV21,
    /// Postman Environment format
    PostmanEnvironment,
    /// Unknown or unsupported format
    Unknown,
}

/// Preview of what will be imported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    /// Detected format name
    pub format: String,
    /// Collection name (if importing a collection)
    pub collection_name: Option<String>,
    /// Environment name (if importing an environment)
    pub environment_name: Option<String>,
    /// Number of requests to be imported
    pub request_count: usize,
    /// Number of folders to be imported
    pub folder_count: usize,
    /// Number of variables to be imported
    pub variable_count: usize,
    /// Warnings generated during preview
    pub warnings: Vec<ImportWarning>,
}

/// Result of a successful import
#[derive(Debug)]
pub struct ImportResult {
    /// Name of the imported collection/environment
    pub name: String,
    /// Number of requests imported
    pub requests_imported: usize,
    /// Number of folders imported
    pub folders_imported: usize,
    /// Number of variables imported
    pub variables_imported: usize,
    /// Warnings generated during import
    pub warnings: Vec<ImportWarning>,
}

/// Main Postman importer
pub struct PostmanImporter {
    config: ImportConfig,
}

impl PostmanImporter {
    /// Create a new importer with default config
    pub fn new() -> Self {
        Self {
            config: ImportConfig::default(),
        }
    }

    /// Create a new importer with custom config
    pub fn with_config(config: ImportConfig) -> Self {
        Self { config }
    }

    /// Validate a file before importing
    pub fn validate_file(&self, content: &str) -> ValidationResult {
        let mut issues = Vec::new();

        // Check file size
        if content.len() > self.config.max_file_size {
            issues.push(format!(
                "File size ({} bytes) exceeds maximum ({} bytes)",
                content.len(),
                self.config.max_file_size
            ));
            return ValidationResult {
                is_valid: false,
                format: ImportFormat::Unknown,
                issues,
            };
        }

        // Try to parse as JSON
        let json: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                issues.push(format!("Invalid JSON: {}", e));
                return ValidationResult {
                    is_valid: false,
                    format: ImportFormat::Unknown,
                    issues,
                };
            }
        };

        // Detect format
        let format = Self::detect_format(&json);

        match format {
            ImportFormat::Unknown => {
                issues.push(
                    "Unknown format: File is not a valid Postman Collection or Environment"
                        .to_string(),
                );
                ValidationResult {
                    is_valid: false,
                    format,
                    issues,
                }
            }
            ImportFormat::PostmanCollectionV21 => {
                // Try to parse as collection
                match serde_json::from_value::<PostmanCollection>(json) {
                    Ok(collection) => {
                        // Check item count
                        let item_count = Self::count_items(&collection.item);
                        if item_count > self.config.max_items {
                            issues.push(format!(
                                "Too many items: {} exceeds maximum of {}",
                                item_count, self.config.max_items
                            ));
                        }
                        ValidationResult {
                            is_valid: issues.is_empty(),
                            format,
                            issues,
                        }
                    }
                    Err(e) => {
                        issues.push(format!("Invalid collection format: {}", e));
                        ValidationResult {
                            is_valid: false,
                            format,
                            issues,
                        }
                    }
                }
            }
            ImportFormat::PostmanEnvironment => {
                // Try to parse as environment
                match serde_json::from_value::<PostmanEnvironment>(json) {
                    Ok(_) => ValidationResult {
                        is_valid: true,
                        format,
                        issues,
                    },
                    Err(e) => {
                        issues.push(format!("Invalid environment format: {}", e));
                        ValidationResult {
                            is_valid: false,
                            format,
                            issues,
                        }
                    }
                }
            }
        }
    }

    /// Preview what will be imported without actually importing
    pub fn preview(&self, content: &str) -> Result<ImportPreview, ImportError> {
        let json: serde_json::Value =
            serde_json::from_str(content).map_err(|e| ImportError::InvalidJson(e.to_string()))?;

        let format = Self::detect_format(&json);

        match format {
            ImportFormat::PostmanCollectionV21 => {
                let collection: PostmanCollection = serde_json::from_value(json)
                    .map_err(|e| ImportError::InvalidFormat(e.to_string()))?;

                let mapped = map_postman_collection(&collection, self.config.max_depth);
                let (request_count, folder_count) = Self::count_mapped_items(&mapped.items);

                Ok(ImportPreview {
                    format: "Postman Collection v2.1".to_string(),
                    collection_name: Some(mapped.name),
                    environment_name: None,
                    request_count,
                    folder_count,
                    variable_count: mapped.variables.len(),
                    warnings: mapped.warnings,
                })
            }
            ImportFormat::PostmanEnvironment => {
                let env: PostmanEnvironment = serde_json::from_value(json)
                    .map_err(|e| ImportError::InvalidFormat(e.to_string()))?;

                let mapped = map_postman_environment(&env);

                Ok(ImportPreview {
                    format: "Postman Environment".to_string(),
                    collection_name: None,
                    environment_name: Some(mapped.name),
                    request_count: 0,
                    folder_count: 0,
                    variable_count: mapped.variables.len(),
                    warnings: mapped.warnings,
                })
            }
            ImportFormat::Unknown => Err(ImportError::InvalidFormat(
                "Unknown format: Not a valid Postman Collection or Environment".to_string(),
            )),
        }
    }

    /// Import a Postman collection
    pub fn import_collection(
        &self,
        content: &str,
        workspace_path: &Path,
    ) -> Result<ImportResult, ImportError> {
        let json: serde_json::Value =
            serde_json::from_str(content).map_err(|e| ImportError::InvalidJson(e.to_string()))?;

        let format = Self::detect_format(&json);

        // Auto-redirect to environment import if detected
        if format == ImportFormat::PostmanEnvironment {
            return self.import_environment(content, workspace_path);
        }

        if format != ImportFormat::PostmanCollectionV21 {
            return Err(ImportError::InvalidFormat(
                "Not a valid Postman Collection v2.1".to_string(),
            ));
        }

        let collection: PostmanCollection =
            serde_json::from_value(json).map_err(|e| ImportError::InvalidFormat(e.to_string()))?;

        let mapped = map_postman_collection(&collection, self.config.max_depth);

        // Check for errors if not skipping
        if !self.config.skip_on_error {
            let stats = WarningStats::from_warnings(&mapped.warnings);
            if stats.has_errors() {
                return Err(ImportError::Aborted);
            }
        }

        // Create collection directory
        let safe_name = Self::sanitize_name(&mapped.name);
        let collection_dir = workspace_path.join("collections").join(&safe_name);
        std::fs::create_dir_all(&collection_dir)?;

        // Create requests directory
        let requests_dir = collection_dir.join("request");
        std::fs::create_dir_all(&requests_dir)?;

        // Write collection.json
        let collection_meta = serde_json::json!({
            "id": uuid::Uuid::now_v7().to_string(),
            "name": mapped.name,
            "description": mapped.description,
            "schema_version": 1,
        });
        std::fs::write(
            collection_dir.join("collection.json"),
            serde_json::to_string_pretty(&collection_meta).unwrap_or_default(),
        )?;

        // Write items recursively
        let (requests_imported, folders_imported) =
            self.write_items(&mapped.items, &requests_dir)?;

        // Write collection variables if any
        let variables_imported = if !mapped.variables.is_empty() {
            let variables_json: Vec<serde_json::Value> = mapped
                .variables
                .iter()
                .map(|v| {
                    serde_json::json!({
                        "name": v.name,
                        "value": v.value,
                        "enabled": v.enabled,
                        "is_secret": v.is_secret,
                    })
                })
                .collect();

            std::fs::write(
                collection_dir.join("variables.json"),
                serde_json::to_string_pretty(&variables_json).unwrap_or_default(),
            )?;
            mapped.variables.len()
        } else {
            0
        };

        Ok(ImportResult {
            name: mapped.name,
            requests_imported,
            folders_imported,
            variables_imported,
            warnings: mapped.warnings,
        })
    }

    /// Import a Postman environment
    pub fn import_environment(
        &self,
        content: &str,
        workspace_path: &Path,
    ) -> Result<ImportResult, ImportError> {
        let env: PostmanEnvironment =
            serde_json::from_str(content).map_err(|e| ImportError::InvalidJson(e.to_string()))?;

        let mapped = map_postman_environment(&env);

        // Create environments directory
        let environments_dir = workspace_path.join("environments");
        std::fs::create_dir_all(&environments_dir)?;

        // Convert variables to Vortex format
        let variables_json: Vec<serde_json::Value> = mapped
            .variables
            .iter()
            .map(|v| {
                serde_json::json!({
                    "name": v.name,
                    "value": v.value,
                    "enabled": v.enabled,
                    "is_secret": v.is_secret,
                })
            })
            .collect();

        // Create environment file
        let env_json = serde_json::json!({
            "id": uuid::Uuid::now_v7().to_string(),
            "name": mapped.name,
            "variables": variables_json,
            "schema_version": 1,
        });

        let safe_name = Self::sanitize_name(&mapped.name);
        let env_path = environments_dir.join(format!("{}.json", safe_name));
        std::fs::write(
            &env_path,
            serde_json::to_string_pretty(&env_json).unwrap_or_default(),
        )?;

        Ok(ImportResult {
            name: mapped.name,
            requests_imported: 0,
            folders_imported: 0,
            variables_imported: mapped.variables.len(),
            warnings: mapped.warnings,
        })
    }

    /// Detect the format of a JSON value
    fn detect_format(json: &serde_json::Value) -> ImportFormat {
        // Check for Postman Collection (has "info" field with collection metadata)
        if json.get("info").is_some() {
            // Check for schema version to confirm it's v2.1
            let schema = json
                .get("info")
                .and_then(|i| i.get("schema"))
                .and_then(|s| s.as_str())
                .unwrap_or("");

            if schema.contains("v2.1") || schema.contains("v2.0") || json.get("item").is_some() {
                return ImportFormat::PostmanCollectionV21;
            }
        }

        // Check for Postman Environment (has "name" and "values" but no "info")
        if json.get("info").is_none() && json.get("name").is_some() && json.get("values").is_some()
        {
            return ImportFormat::PostmanEnvironment;
        }

        ImportFormat::Unknown
    }

    /// Count items recursively
    fn count_items(items: &[super::types::PostmanItem]) -> usize {
        let mut count = items.len();
        for item in items {
            if let Some(ref sub_items) = item.item {
                count += Self::count_items(sub_items);
            }
        }
        count
    }

    /// Count mapped items
    fn count_mapped_items(items: &[MappedItem]) -> (usize, usize) {
        let mut requests = 0;
        let mut folders = 0;

        for item in items {
            match item {
                MappedItem::Request(_) => requests += 1,
                MappedItem::Folder(f) => {
                    folders += 1;
                    let (sub_req, sub_fold) = Self::count_mapped_items(&f.items);
                    requests += sub_req;
                    folders += sub_fold;
                }
            }
        }

        (requests, folders)
    }

    /// Sanitize a name for use as a filename/directory name
    fn sanitize_name(name: &str) -> String {
        name.to_lowercase()
            .replace(' ', "-")
            .replace('/', "-")
            .replace('\\', "-")
            .replace(':', "-")
            .replace('*', "-")
            .replace('?', "-")
            .replace('"', "-")
            .replace('<', "-")
            .replace('>', "-")
            .replace('|', "-")
    }

    /// Write items to filesystem recursively
    fn write_items(&self, items: &[MappedItem], dir: &Path) -> Result<(usize, usize), ImportError> {
        let mut requests = 0;
        let mut folders = 0;

        for item in items {
            match item {
                MappedItem::Request(req) => {
                    let mut request_json = serde_json::json!({
                        "id": req.id,
                        "name": req.name,
                        "method": req.method,
                        "url": req.url,
                        "headers": req.headers,
                        "schema_version": 1,
                    });

                    if let Some(ref desc) = req.description {
                        request_json["description"] = serde_json::json!(desc);
                    }

                    if !req.query_params.is_empty() {
                        request_json["query_params"] = serde_json::json!(req.query_params);
                    }

                    if let Some(ref body) = req.body {
                        request_json["body"] = body_to_vortex_json(body);
                    }

                    if let Some(ref auth) = req.auth {
                        request_json["auth"] = auth_to_vortex_json(auth);
                    }

                    let safe_name = Self::sanitize_name(&req.name);
                    let file_path = dir.join(format!("{}.json", safe_name));
                    std::fs::write(
                        &file_path,
                        serde_json::to_string_pretty(&request_json).unwrap_or_default(),
                    )?;
                    requests += 1;
                }
                MappedItem::Folder(folder) => {
                    let safe_name = Self::sanitize_name(&folder.name);
                    let folder_dir = dir.join(&safe_name);
                    std::fs::create_dir_all(&folder_dir)?;

                    // Write folder.json with display name
                    let folder_meta = serde_json::json!({
                        "name": folder.name,
                        "description": folder.description,
                        "schema_version": 1,
                    });
                    std::fs::write(
                        folder_dir.join("folder.json"),
                        serde_json::to_string_pretty(&folder_meta).unwrap_or_default(),
                    )?;

                    let (sub_req, sub_fold) = self.write_items(&folder.items, &folder_dir)?;
                    requests += sub_req;
                    folders += 1 + sub_fold;
                }
            }
        }

        Ok((requests, folders))
    }
}

impl Default for PostmanImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_collection_format() {
        let json = serde_json::json!({
            "info": {
                "name": "Test",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": []
        });

        assert_eq!(
            PostmanImporter::detect_format(&json),
            ImportFormat::PostmanCollectionV21
        );
    }

    #[test]
    fn test_detect_environment_format() {
        let json = serde_json::json!({
            "name": "Test Env",
            "values": []
        });

        assert_eq!(
            PostmanImporter::detect_format(&json),
            ImportFormat::PostmanEnvironment
        );
    }

    #[test]
    fn test_validate_valid_collection() {
        let content = r#"{
            "info": {"name": "Test", "schema": "v2.1"},
            "item": []
        }"#;

        let importer = PostmanImporter::new();
        let result = importer.validate_file(content);
        assert!(result.is_valid);
        assert_eq!(result.format, ImportFormat::PostmanCollectionV21);
    }

    #[test]
    fn test_validate_oversized_file() {
        let config = ImportConfig {
            max_file_size: 100,
            ..Default::default()
        };
        let importer = PostmanImporter::with_config(config);

        let content = "x".repeat(200);
        let result = importer.validate_file(&content);
        assert!(!result.is_valid);
        assert!(result.issues[0].contains("exceeds maximum"));
    }

    #[test]
    fn test_preview_collection() {
        let content = r#"{
            "info": {"name": "My API", "schema": "v2.1"},
            "item": [
                {"name": "Get Users", "request": {"method": "GET", "url": "https://api.example.com/users"}},
                {"name": "Auth", "item": [
                    {"name": "Login", "request": {"method": "POST", "url": "https://api.example.com/login"}}
                ]}
            ],
            "variable": [{"key": "baseUrl", "value": "https://api.example.com"}]
        }"#;

        let importer = PostmanImporter::new();
        let preview = importer.preview(content).unwrap();

        assert_eq!(preview.collection_name, Some("My API".to_string()));
        assert_eq!(preview.request_count, 2);
        assert_eq!(preview.folder_count, 1);
        assert_eq!(preview.variable_count, 1);
    }

    #[test]
    fn test_import_collection() {
        let content = r#"{
            "info": {"name": "Test API", "schema": "v2.1"},
            "item": [
                {"name": "Get Users", "request": {"method": "GET", "url": "https://api.example.com/users"}}
            ]
        }"#;

        let temp_dir = TempDir::new().unwrap();
        let importer = PostmanImporter::new();
        let result = importer
            .import_collection(content, temp_dir.path())
            .unwrap();

        assert_eq!(result.name, "Test API");
        assert_eq!(result.requests_imported, 1);

        // Verify files were created
        let collection_dir = temp_dir.path().join("collections").join("test-api");
        assert!(collection_dir.exists());
        assert!(collection_dir.join("collection.json").exists());
        assert!(
            collection_dir
                .join("request")
                .join("get-users.json")
                .exists()
        );
    }

    #[test]
    fn test_import_environment() {
        let content = r#"{
            "name": "Development",
            "values": [
                {"key": "BASE_URL", "value": "https://dev.api.com", "enabled": true},
                {"key": "API_KEY", "value": "secret", "type": "secret"}
            ]
        }"#;

        let temp_dir = TempDir::new().unwrap();
        let importer = PostmanImporter::new();
        let result = importer
            .import_environment(content, temp_dir.path())
            .unwrap();

        assert_eq!(result.name, "Development");
        assert_eq!(result.variables_imported, 2);

        // Verify file was created
        let env_path = temp_dir
            .path()
            .join("environments")
            .join("development.json");
        assert!(env_path.exists());
    }
}
