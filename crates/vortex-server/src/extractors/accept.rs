use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};

/// Formatos de salida soportados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Json,
    Yaml,
    Properties,
}

impl OutputFormat {
    /// Determina el formato basado en el header Accept.
    pub fn from_accept(accept: Option<&str>) -> Self {
        match accept {
            None => Self::Json,
            Some(accept) => {
                let accept = accept.to_lowercase();

                if accept.contains("application/x-yaml")
                    || accept.contains("text/yaml")
                    || accept.contains("application/yaml")
                {
                    Self::Yaml
                } else if accept.contains("text/plain") {
                    Self::Properties
                } else {
                    // Default to JSON for application/json, */*, or unknown
                    Self::Json
                }
            },
        }
    }

    /// Retorna el Content-Type correspondiente.
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Yaml => "application/x-yaml",
            Self::Properties => "text/plain; charset=utf-8",
        }
    }
}

/// Extractor que parsea el header Accept.
pub struct AcceptFormat(pub OutputFormat);

impl<S> FromRequestParts<S> for AcceptFormat
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let accept = parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok());

        Ok(AcceptFormat(OutputFormat::from_accept(accept)))
    }
}
