//! Integration test to verify the workspace compiles correctly.

#![allow(clippy::no_effect_underscore_binding)]

#[test]
fn domain_crate_compiles() {
    // Verify domain types are accessible
    let _method = vortex_domain::request::HttpMethod::Get;
    let _request = vortex_domain::request::RequestSpec::new("Test");
    let _collection = vortex_domain::collection::Collection::new("Test");
    let _env = vortex_domain::environment::Environment::new("Test");
}

#[test]
fn application_crate_compiles() {
    // Verify application types are accessible
    let _error = vortex_application::ApplicationError::Timeout;
}

#[test]
fn infrastructure_crate_compiles() {
    // Verify infrastructure adapters are accessible
    use vortex_application::ports::Clock;
    let clock = vortex_infrastructure::adapters::SystemClock::new();
    let _now = clock.now();
}
