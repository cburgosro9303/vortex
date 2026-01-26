//! HTTP Request domain types

mod body;
mod header;
mod method;
mod spec;

pub use body::{RequestBody, RequestBodyKind};
pub use header::{Header, Headers};
pub use method::HttpMethod;
pub use spec::RequestSpec;
