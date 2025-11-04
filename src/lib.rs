//! # ic-dev-kit-rs
//!
//! A comprehensive Rust toolkit for Internet Computer canister development.
//!
//! ## Features
//!
//! - **Authentication**: Simple principal-based authorization with guards
//! - **HTTP Handling**: Request/response utilities, routing, and JSON handling
//! - **Telemetry**: Canistergeek integration for monitoring and logging
//! - **Storage**: Large object storage with chunking support
//!
//! ## Quick Start
//!
//! ### Authentication
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::auth;
//!
//! #[ic_cdk::init]
//! fn init() {
//!     auth::init_with_caller();
//! }
//!
//! #[ic_cdk::update(guard = "auth::is_authorized")]
//! fn protected_method() {
//!     // Only authorized principals can call this
//! }
//! ```
//!
//! ### HTTP Handling
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::http;
//!
//! #[ic_cdk::query]
//! fn http_request(req: http::HttpRequest) -> http::HttpResponse {
//!     let path = http::extract_path(&req.url);
//!
//!     match path {
//!         "/api/hello" => http::success_response(&"Hello, IC!").unwrap(),
//!         _ => http::HttpError::NotFound.to_response(),
//!     }
//! }
//! ```
//!
//! ### Telemetry
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::telemetry;
//!
//! #[ic_cdk::init]
//! fn init() {
//!     telemetry::init();
//! }
//!
//! #[ic_cdk::update]
//! fn do_work() {
//!     telemetry::track_metrics();
//!     telemetry::log_info("Work completed");
//! }
//! ```
//!
//! ### Storage
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::storage;
//!
//! #[ic_cdk::init]
//! fn init() {
//!     storage::init();
//! }
//!
//! #[ic_cdk::update]
//! fn store_file(id: String, data: Vec<u8>) {
//!     storage::store(id, data, Some("application/octet-stream".to_string())).unwrap();
//! }
//! ```

pub mod auth;
pub mod http;
pub mod telemetry;
pub mod storage;

// Re-export common types for convenience
pub use candid::Principal;

/// Prelude module with commonly used imports
pub mod prelude {
    pub use crate::auth::{self, AuthError, AuthResult};
    pub use crate::http::{self, HttpError, HttpRequest, HttpResponse, HttpResult, HttpMethod};
    pub use crate::telemetry::{self, TelemetryError, TelemetryResult};
    pub use crate::storage::{self, StorageError, StorageResult, ObjectMetadata};
    pub use candid::Principal;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}