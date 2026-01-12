use vortex_core::{Application, ConfigMap, PropertySource, Result, VortexError};

#[test]
fn test_validation_workflow() {
    fn validate_and_process(app_name: &str) -> Result<String> {
        if app_name.is_empty() {
            return Err(VortexError::invalid_application(
                app_name,
                "Application name cannot be empty",
            ));
        }
        let app = Application::new(app_name);
        Ok(format!("Processed: {}", app))
    }

    // Valid case
    assert!(validate_and_process("myapp").is_ok());

    // Invalid case
    let result = validate_and_process("");
    assert!(result.is_err());

    if let Err(VortexError::InvalidApplication { name, reason }) = result {
        assert!(name.is_empty());
        assert!(reason.contains("empty"));
    } else {
        panic!("Expected InvalidApplication error");
    }
}

#[test]
fn test_error_context_preservation() {
    fn load_config() -> Result<ConfigMap> {
        // Simular un error de parsing
        Err(VortexError::parse_error(
            "application.yml",
            "Invalid YAML syntax at line 10",
        ))
    }

    let result = load_config();
    assert!(result.is_err());

    let error = result.unwrap_err();
    let message = format!("{}", error);

    assert!(message.contains("application.yml"));
    assert!(message.contains("line 10"));
}

#[test]
fn test_config_value_workflow() {
    // 1. Create ConfigMap from JSON (simulating loading from file)
    let json_config = r#"{
        "server": {
            "port": 8000,
            "host": "localhost"
        },
        "app": {
            "name": "Default App",
            "enabled": true
        },
        "features": ["auth", "monitoring"]
    }"#;
    
    let config = ConfigMap::from_json(json_config).expect("Failed to parse JSON");

    // 2. Verify typed access via dot notation
    assert_eq!(config.get("server.port").unwrap().as_i64(), Some(8000));
    assert_eq!(config.get("server.host").unwrap().as_str(), Some("localhost"));
    assert_eq!(config.get("app.enabled").unwrap().as_bool(), Some(true));
    
    // Verify array access (requires manual traversing for now or specialized get)
    if let Some(features) = config.get("features").and_then(|v| v.as_array()) {
        assert_eq!(features.len(), 2);
        assert_eq!(features[0].as_str(), Some("auth"));
    } else {
        panic!("Features should be an array");
    }

    // 3. Create PropertySource wrapping the config
    let source = PropertySource::new("application.yml", config);
    
    assert_eq!(source.name, "application.yml");
    assert!(!source.config.is_empty());
}

#[test]
fn test_error_propagation_with_question_mark() {
    fn step1() -> Result<()> {
        Err(VortexError::source_error("git", "connection timeout"))
    }

    fn step2() -> Result<()> {
        step1()?; // Should propagate
        Ok(())
    }

    fn step3() -> Result<String> {
        step2()?;
        Ok("success".into())
    }

    let result = step3();
    assert!(result.is_err());
    assert!(result.unwrap_err().is_source_error());
}
