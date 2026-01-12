use std::collections::HashMap;
use vortex_core::{Application, ConfigMap, Label, Profile, PropertySource};

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
