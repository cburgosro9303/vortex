use std::collections::HashMap;
use vortex_core::{Application, ConfigMap, Label, Profile, PropertySource, Result, VortexError};

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
fn test_complete_config_workflow() {
    // Simulate loading configuration from multiple sources
    let app_props = PropertySource::new(
        "application.yml",
        HashMap::from([
            ("server.port".into(), "8000".into()),
            ("app.name".into(), "Default App".into()),
        ]),
    );

    let profile_props = PropertySource::new(
        "application-production.yml",
        HashMap::from([
            ("server.port".into(), "8080".into()),
            ("database.pool.size".into(), "20".into()),
        ]),
    );

    let config = ConfigMap::builder()
        .application(Application::new("myapp"))
        .profile(Profile::new("production"))
        .label(Label::new("main"))
        .property_source(profile_props) // Higher priority
        .property_source(app_props) // Lower priority
        .build();

    // Profile-specific value takes precedence
    assert_eq!(config.get_property("server.port"), Some("8080"));

    // Falls back to default
    assert_eq!(config.get_property("app.name"), Some("Default App"));

    // Profile-specific only
    assert_eq!(config.get_property("database.pool.size"), Some("20"));
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
