//! # ic-dev-kit-rs
//!
//! A comprehensive Rust toolkit for Internet Computer canister development.

pub mod auth;
pub mod http;
pub mod telemetry;
pub mod storage;
pub mod large_objects;
pub mod intercanister;
pub mod candle;

// Re-export common types for convenience
pub use candid::Principal;

/// Prelude module with commonly used imports
pub mod prelude {
    pub use crate::auth::{self, AuthError, AuthResult};
    pub use crate::http::{self, HttpError, HttpRequest, HttpResponse, HttpResult, HttpMethod};
    pub use crate::telemetry::{self, TelemetryError, TelemetryResult};
    pub use crate::storage::{self, StorageRegistry};
    pub use crate::large_objects;
    pub use crate::intercanister;
    pub use crate::candle::{self, CandleModel, AutoregressiveModel, GenerationConfig};
    pub use candid::Principal;
}
