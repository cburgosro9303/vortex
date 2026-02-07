//! Vortex Domain - Core business types
//!
//! This crate defines the domain model for the Vortex API Client.
//! All types here are pure Rust with no I/O dependencies.

pub mod auth;
pub mod codegen;
pub mod collection;
pub mod cookie;
pub mod environment;
pub mod error;
pub mod export;
pub mod history;
pub mod id;
pub mod persistence;
pub mod proxy;
pub mod request;
pub mod response;
pub mod scripting;
pub mod settings;
pub mod state;
pub mod testing;
pub mod tls;
pub mod websocket;

pub use auth::{ApiKeyLocation, AuthConfig, AuthError, AuthResolution, OAuth2Token};
pub use codegen::{CodeGenOptions, CodeLanguage, CodeSnippet};
pub use cookie::{Cookie, CookieJar, SameSite};
pub use error::{DomainError, DomainResult};
pub use export::{ExportFormat, ExportOptions, ExportResult, ExportWarning};
pub use history::{HistoryAuth, HistoryEntry, HistoryHeader, HistoryParam, RequestHistory};
pub use id::{generate_id, generate_id_v7};
pub use proxy::{GlobalProxySettings, ProxyConfig, ProxyError, ProxyType};
pub use scripting::{RequestScripts, Script, ScriptCommand, ScriptLanguage, ScriptResult};
pub use settings::{FontScale, ThemeMode, UserSettings};
pub use state::{RequestErrorKind, RequestState};
pub use testing::{
    Assertion, AssertionResult, ComparisonOperator, StatusExpectation, TestResults, TestSuite,
};
pub use tls::{
    CertificateInfo, CertificateSource, ClientCertificate, PrivateKeySource, TlsConfig,
    TlsSecurityWarning, TlsVersion, WarningSeverity,
};
pub use websocket::{
    ConnectionInfo, ConnectionState, MessageDirection, MessageType, WebSocketConfig,
    WebSocketError, WebSocketMessage,
};
