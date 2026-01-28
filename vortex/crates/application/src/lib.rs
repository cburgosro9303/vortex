//! Vortex Application - Use cases and ports
//!
//! This crate defines the application layer with:
//! - Port traits (interfaces for external dependencies)
//! - Use case orchestration
//! - Application-level error handling
//! - Variable resolution engine
//! - Authentication providers and token management

pub mod auth;
pub mod error;
pub mod execute_request;
pub mod ports;
pub mod use_cases;
pub mod variable_resolver;

pub use auth::{AuthEvent, AuthProvider, AuthorizationState, TokenStatus, TokenStore};
pub use error::{ApplicationError, ApplicationResult};
pub use execute_request::{ExecuteRequest, ExecuteRequestError, ExecuteResult, ExecuteResultExt};
pub use ports::{
    CancellationReceiver, CancellationToken, EnvironmentError, EnvironmentRepository, HttpClient,
    HttpClientError, SecretsError, SecretsRepository,
};
pub use use_cases::{
    CreateRequest, CreateRequestInput, CreateRequestOutput, CreateWorkspace, CreateWorkspaceInput,
    ListEnvironments, ListEnvironmentsOutput, LoadCollection, LoadCollectionInput, LoadEnvironment,
    LoadEnvironmentError, LoadEnvironmentOutput, ResolveVariables, ResolveVariablesOutput,
    SaveCollection, SaveCollectionInput, SaveEnvironment, SaveEnvironmentError, SwitchEnvironment,
    SwitchEnvironmentError, SwitchEnvironmentOutput, UpdateRequest, UpdateRequestInput,
    UpdateRequestOutput,
};
pub use variable_resolver::{
    BuiltinInfo, BuiltinVariables, ResolutionResult, VariableReference, VariableResolver,
};
