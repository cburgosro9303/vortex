//! Metrics endpoint handler.

use axum::{extract::State, response::IntoResponse};
use metrics_exporter_prometheus::PrometheusHandle;

/// Handler para el endpoint /metrics
pub async fn metrics_handler(State(prometheus): State<PrometheusHandle>) -> impl IntoResponse {
    prometheus.render()
}
