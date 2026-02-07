//! Export infrastructure.
//!
//! This module provides exporters for various formats.

mod har;
mod openapi;

pub use har::HarExporter;
pub use openapi::OpenApiExporter;

use thiserror::Error;
use vortex_domain::export::{ExportFormat, ExportOptions, ExportResult};
use vortex_domain::request::RequestSpec;
use vortex_domain::response::ResponseSpec;

/// Export error type.
#[derive(Debug, Error)]
pub enum ExportError {
    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Unsupported format.
    #[error("Unsupported export format: {format:?}")]
    UnsupportedFormat {
        /// The unsupported format.
        format: ExportFormat,
    },
    /// Invalid request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

/// Export a single request.
///
/// # Errors
///
/// Returns an error if the export fails.
pub fn export_request(
    request: &RequestSpec,
    response: Option<&ResponseSpec>,
    options: &ExportOptions,
) -> Result<ExportResult, ExportError> {
    export_requests(
        std::slice::from_ref(request),
        &response.into_iter().cloned().collect::<Vec<_>>(),
        options,
    )
}

/// Export multiple requests.
///
/// # Errors
///
/// Returns an error if the export fails.
pub fn export_requests(
    requests: &[RequestSpec],
    responses: &[ResponseSpec],
    options: &ExportOptions,
) -> Result<ExportResult, ExportError> {
    match options.format {
        ExportFormat::Har => HarExporter::export(requests, responses, options),
        ExportFormat::OpenApi3 => OpenApiExporter::export(requests, options),
        ExportFormat::Curl => {
            // Use the code generator for cURL
            let content = requests
                .iter()
                .map(|req| {
                    let code_options = vortex_domain::codegen::CodeGenOptions {
                        language: vortex_domain::codegen::CodeLanguage::Curl,
                        ..Default::default()
                    };
                    crate::codegen::generate_code(req, &code_options).code
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            Ok(ExportResult::new(
                content,
                ExportFormat::Curl,
                requests.len(),
            ))
        }
        format => Err(ExportError::UnsupportedFormat { format }),
    }
}
