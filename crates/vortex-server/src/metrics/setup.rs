//! Metrics setup and initialization.

use metrics_exporter_prometheus::PrometheusHandle;
use tracing::info;

/// Inicializa el sistema de metricas y retorna el handle para el endpoint.
pub fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();

    // Configurar buckets para histogramas (en segundos)
    let handle = builder
        .set_buckets(&[
            0.0001, // 100 microsegundos
            0.0005, // 500 microsegundos
            0.001,  // 1 milisegundo
            0.0025, // 2.5 milisegundos
            0.005,  // 5 milisegundos
            0.01,   // 10 milisegundos
            0.025,  // 25 milisegundos
            0.05,   // 50 milisegundos
            0.1,    // 100 milisegundos
            0.25,   // 250 milisegundos
            0.5,    // 500 milisegundos
            1.0,    // 1 segundo
            2.5,    // 2.5 segundos
            5.0,    // 5 segundos
            10.0,   // 10 segundos
        ])
        .expect("failed to set histogram buckets")
        .install_recorder()
        .expect("failed to install metrics recorder");

    info!("Metrics system initialized");
    handle
}
