//! # ic-dev-kit-rs
//!
//! A comprehensive Rust toolkit for Internet Computer canister development.
//!
//! ## Features
//!
//! - **Authentication**: Simple principal-based authorization with guards
//! - **HTTP Handling**: Request/response utilities, routing, and JSON handling
//! - **Telemetry**: Canistergeek integration for monitoring and logging
//! - **Storage**: Type-safe wrappers for IC stable storage (StableBTreeMap)
//! - **Large Objects**: Chunked upload system for large files (ML models, media, etc.)
//! - **Inter-canister Calls**: Safe wrappers with timeout, retries, and logging
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
//! use ic_stable_structures::{StableBTreeMap, DefaultMemoryImpl};
//!
//! // Define your REGISTRIES in your canister
//! thread_local! {
//!     static REGISTRIES: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = ...;
//! }
//!
//! // Save data
//! storage::with_registry_mut(&REGISTRIES, |reg| {
//!     storage::save_data(reg, "my_key", &my_data)
//! });
//!
//! // Load data
//! let data: Option<MyType> = storage::with_registry_ref(&REGISTRIES, |reg| {
//!     storage::load_data(reg, "my_key")
//! });
//! ```
//!
//! ### Large Objects
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::large_objects;
//!
//! #[ic_cdk::update]
//! fn upload_chunk(chunk_id: u32, data: Vec<u8>) {
//!     large_objects::append_parallel_chunk(chunk_id, data);
//! }
//!
//! #[ic_cdk::update]
//! fn finalize_upload() -> Result<Vec<u8>, String> {
//!     large_objects::consolidate_parallel_chunks()?;
//!     Ok(large_objects::get_buffer_data())
//! }
//! ```
//!
//! ### Inter-canister Calls
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::intercanister;
//!
//! #[ic_cdk::update]
//! async fn call_other_canister() -> Result<String, String> {
//!     let result: String = intercanister::call(
//!         other_canister_id,
//!         "get_data",
//!         (),
//!     ).await?;
//!
//!     Ok(result)
//! }
//! ```

pub mod auth;
pub mod http;
pub mod telemetry;
pub mod storage;
pub mod large_objects;
pub mod intercanister;

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
    pub use candid::Principal;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}