//! Infrastructure adapters

mod reqwest_client;
mod system_clock;

pub use reqwest_client::ReqwestHttpClient;
pub use system_clock::SystemClock;
