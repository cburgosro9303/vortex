//! TLS configuration domain types for Vortex API Client.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TLS configuration for HTTP requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TlsConfig {
    /// Whether to verify server certificates.
    #[serde(default = "default_true")]
    pub verify_certificates: bool,

    /// Custom CA certificates to trust.
    #[serde(default)]
    pub ca_certificates: Vec<CertificateSource>,

    /// Client certificate for mTLS.
    #[serde(default)]
    pub client_certificate: Option<ClientCertificate>,

    /// Minimum TLS version (default: TLS 1.2).
    #[serde(default)]
    pub min_tls_version: Option<TlsVersion>,

    /// Accept invalid/self-signed certificates (dangerous!).
    #[serde(default)]
    pub danger_accept_invalid_certs: bool,

    /// Accept invalid hostnames (dangerous!).
    #[serde(default)]
    pub danger_accept_invalid_hostnames: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            verify_certificates: true,
            ca_certificates: Vec::new(),
            client_certificate: None,
            min_tls_version: None,
            danger_accept_invalid_certs: false,
            danger_accept_invalid_hostnames: false,
        }
    }
}

const fn default_true() -> bool {
    true
}

impl TlsConfig {
    /// Create a new default TLS config with certificate verification enabled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an insecure config that accepts any certificate.
    /// WARNING: This should only be used for testing!
    #[must_use]
    pub fn insecure() -> Self {
        Self {
            verify_certificates: false,
            danger_accept_invalid_certs: true,
            danger_accept_invalid_hostnames: true,
            ..Default::default()
        }
    }

    /// Check if this config uses any dangerous/insecure options.
    #[must_use]
    pub fn security_warnings(&self) -> Vec<TlsSecurityWarning> {
        let mut warnings = vec![];

        if !self.verify_certificates {
            warnings.push(TlsSecurityWarning::CertificateVerificationDisabled);
        }

        if self.danger_accept_invalid_certs {
            warnings.push(TlsSecurityWarning::AcceptingInvalidCertificates);
        }

        if self.danger_accept_invalid_hostnames {
            warnings.push(TlsSecurityWarning::AcceptingInvalidHostnames);
        }

        warnings
    }

    /// Check if this is a secure configuration.
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.security_warnings().is_empty()
    }

    /// Add a CA certificate from a PEM file.
    #[must_use]
    pub fn with_ca_pem_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_certificates
            .push(CertificateSource::PemFile { path: path.into() });
        self
    }

    /// Add a client certificate for mTLS.
    #[must_use]
    pub fn with_client_cert(mut self, cert: ClientCertificate) -> Self {
        self.client_certificate = Some(cert);
        self
    }

    /// Set minimum TLS version.
    #[must_use]
    pub const fn with_min_tls_version(mut self, version: TlsVersion) -> Self {
        self.min_tls_version = Some(version);
        self
    }
}

/// Source for a certificate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CertificateSource {
    /// Load from a PEM file.
    PemFile {
        /// Path to the PEM file.
        path: PathBuf,
    },
    /// Load from a DER file.
    DerFile {
        /// Path to the DER file.
        path: PathBuf,
    },
    /// Inline PEM content.
    PemContent {
        /// PEM-encoded certificate content.
        content: String,
    },
    /// Use system certificate store.
    System,
}

/// Client certificate for mTLS authentication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientCertificate {
    /// Certificate source.
    pub certificate: CertificateSource,
    /// Private key source.
    pub private_key: PrivateKeySource,
    /// Password for encrypted keys (if applicable).
    #[serde(default)]
    pub password: Option<String>,
}

impl ClientCertificate {
    /// Create from PEM files.
    #[must_use]
    pub fn from_pem_files(cert_path: impl Into<PathBuf>, key_path: impl Into<PathBuf>) -> Self {
        Self {
            certificate: CertificateSource::PemFile {
                path: cert_path.into(),
            },
            private_key: PrivateKeySource::PemFile {
                path: key_path.into(),
            },
            password: None,
        }
    }

    /// Create from a PKCS#12 file.
    #[must_use]
    pub fn from_pkcs12(path: impl Into<PathBuf>, password: Option<String>) -> Self {
        Self {
            certificate: CertificateSource::PemFile {
                path: PathBuf::new(),
            }, // Not used for PKCS12
            private_key: PrivateKeySource::Pkcs12File { path: path.into() },
            password,
        }
    }
}

/// Source for a private key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PrivateKeySource {
    /// Load from a PEM file.
    PemFile {
        /// Path to the PEM file.
        path: PathBuf,
    },
    /// Load from a DER file.
    DerFile {
        /// Path to the DER file.
        path: PathBuf,
    },
    /// Load from a PKCS#12 file (includes cert).
    Pkcs12File {
        /// Path to the PKCS#12 file.
        path: PathBuf,
    },
    /// Inline PEM content.
    PemContent {
        /// PEM-encoded private key content.
        content: String,
    },
}

/// TLS protocol version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[derive(Default)]
pub enum TlsVersion {
    /// TLS 1.0 (deprecated, avoid if possible)
    #[serde(rename = "1.0")]
    Tls10,
    /// TLS 1.1 (deprecated, avoid if possible)
    #[serde(rename = "1.1")]
    Tls11,
    /// TLS 1.2 (recommended minimum)
    #[serde(rename = "1.2")]
    #[default]
    Tls12,
    /// TLS 1.3 (most secure)
    #[serde(rename = "1.3")]
    Tls13,
}


/// TLS security warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsSecurityWarning {
    /// Certificate verification is disabled.
    CertificateVerificationDisabled,
    /// Accepting invalid certificates.
    AcceptingInvalidCertificates,
    /// Accepting invalid hostnames.
    AcceptingInvalidHostnames,
}

impl TlsSecurityWarning {
    /// Get a user-friendly message for this warning.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::CertificateVerificationDisabled => {
                "Certificate verification is disabled. This makes connections vulnerable to \
                 man-in-the-middle attacks."
            }
            Self::AcceptingInvalidCertificates => {
                "Accepting invalid certificates. Connections may be intercepted by attackers."
            }
            Self::AcceptingInvalidHostnames => {
                "Accepting invalid hostnames. The server identity is not being verified."
            }
        }
    }

    /// Get the severity level.
    #[must_use]
    pub const fn severity(&self) -> WarningSeverity {
        WarningSeverity::High
    }
}

/// Severity level for warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    /// Low severity.
    Low,
    /// Medium severity.
    Medium,
    /// High severity.
    High,
}

/// Information about a loaded certificate.
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Subject common name.
    pub subject_cn: Option<String>,
    /// Issuer common name.
    pub issuer_cn: Option<String>,
    /// Serial number (hex).
    pub serial_number: String,
    /// Not valid before.
    pub not_before: chrono::DateTime<chrono::Utc>,
    /// Not valid after.
    pub not_after: chrono::DateTime<chrono::Utc>,
    /// Whether this is a CA certificate.
    pub is_ca: bool,
    /// Fingerprint (SHA-256).
    pub fingerprint_sha256: String,
}

impl CertificateInfo {
    /// Check if the certificate is currently valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now();
        now >= self.not_before && now <= self.not_after
    }

    /// Days until expiry (negative if expired).
    #[must_use]
    pub fn days_until_expiry(&self) -> i64 {
        (self.not_after - chrono::Utc::now()).num_days()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tls_config_is_secure() {
        let config = TlsConfig::default();
        assert!(config.is_secure());
        assert!(config.security_warnings().is_empty());
    }

    #[test]
    fn test_insecure_tls_config() {
        let config = TlsConfig::insecure();
        assert!(!config.is_secure());
        assert_eq!(config.security_warnings().len(), 3);
    }

    #[test]
    fn test_tls_version_default() {
        assert_eq!(TlsVersion::default(), TlsVersion::Tls12);
    }

    #[test]
    fn test_client_certificate_from_pem_files() {
        let cert = ClientCertificate::from_pem_files("/path/to/cert.pem", "/path/to/key.pem");
        match cert.certificate {
            CertificateSource::PemFile { path } => {
                assert_eq!(path, PathBuf::from("/path/to/cert.pem"));
            }
            _ => panic!("Expected PemFile"),
        }
    }

    #[test]
    fn test_tls_config_builder() {
        let config = TlsConfig::new()
            .with_ca_pem_file("/ca.pem")
            .with_min_tls_version(TlsVersion::Tls13);

        assert_eq!(config.ca_certificates.len(), 1);
        assert_eq!(config.min_tls_version, Some(TlsVersion::Tls13));
    }
}
