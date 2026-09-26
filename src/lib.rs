//! # ic-dev-kit-rs
//!
//! A Rust toolkit for Internet Computer canister development that standardizes
//! common patterns: authentication, HTTP handling, storage, telemetry, and more.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::prelude::*;
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
//! ## Feature Flags
//!
//! | Feature | Description | Dependencies |
//! |---------|-------------|--------------|
//! | `storage` | Stable storage utilities | `ic-stable-structures` |
//! | `telemetry` | Canistergeek monitoring/logging | `canistergeek_ic_rust` |
//! | `candle` | ML model traits and GGUF helpers | `candle-core`, `candle-nn` |
//! | `text-generation` | LLM generation loop and tokenizer helpers | `candle`, `candle-transformers`, `tokenizers` |
//!
//! ## Modules
//!
//! - [`auth`] - Principal-based authorization with guard functions
//! - [`http`] - HTTP request/response types, routing, and gateway streaming
//! - [`large_objects`] - Chunked uploads for large files, per-caller and capped
//! - [`intercanister`] - Inter-canister call wrappers with logging
//! - [`prelude`] - Glob-import of the commonly used items
//! - `storage` - Type-safe stable storage (requires `storage`)
//! - `telemetry` - Canistergeek integration (requires `telemetry`)
//! - `candle` - ML model traits and GGUF loading (requires `candle`)
//! - `text_generation` - LLM generation loop (requires `text-generation`)
//! - `model_server` - Ready-made LLM server and endpoint macro (requires
//!   `text-generation` **and** `storage`)
//!
//! Endpoint macros: `export_auth_endpoints!`, `export_telemetry_endpoints!`,
//! `generate_upload_endpoints!`, `generate_model_endpoints!`. Invoke
//! `ic_cdk::export_candid!()` in the same module as the macros.
//!
//! (Feature-gated modules are not linked so `cargo doc` succeeds under any
//! feature set.)

pub mod auth;
pub mod http;
pub mod intercanister;
pub mod large_objects;

#[cfg(feature = "telemetry")]
pub mod telemetry;

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "candle")]
pub mod candle;

#[cfg(feature = "text-generation")]
pub mod text_generation;

#[cfg(all(feature = "text-generation", feature = "storage"))]
pub mod model_server;

/// Support items for the exported macros. Not part of the public API.
///
/// A `#[cfg(feature = "telemetry")]` written inside a `macro_rules!` body is
/// evaluated against the features of the crate that *expands* the macro (the
/// consumer), not this crate's. The macros therefore call these shims, which
/// are resolved here and become no-ops when `telemetry` is disabled.
#[doc(hidden)]
pub mod __private {
    #[cfg(feature = "telemetry")]
    pub use crate::telemetry::{collect_metrics, log_error, log_info};

    #[cfg(not(feature = "telemetry"))]
    pub fn collect_metrics() {}

    #[cfg(not(feature = "telemetry"))]
    pub fn log_info(_message: impl Into<String>) {}

    #[cfg(not(feature = "telemetry"))]
    pub fn log_error(_message: impl Into<String>) {}
}

pub use candid::Principal;

/// Prelude module
pub mod prelude {
    pub use crate::auth::{self, AuthError, AuthResult};
    pub use crate::http::{
        self, HttpError, HttpMethod, HttpRequest, HttpResponse, HttpResult, StreamingCallback,
        StreamingCallbackHttpResponse, StreamingCallbackToken, StreamingStrategy,
    };
    pub use crate::intercanister;
    pub use crate::large_objects;
    pub use candid::Principal;

    #[cfg(feature = "telemetry")]
    pub use crate::telemetry;

    #[cfg(feature = "storage")]
    pub use crate::storage::{self, StorageRegistry};

    #[cfg(feature = "candle")]
    pub use crate::candle::{self, gguf, CandleModel, ModelManager, ModelMetadata};

    #[cfg(feature = "text-generation")]
    pub use crate::text_generation::{
        self, format_generation_stats, generate_autoregressive, tokenizer, AutoregressiveModel,
        GenerationConfig, GenerationResponse, StopReason, TokenizerHandle,
    };

    #[cfg(all(feature = "text-generation", feature = "storage"))]
    pub use crate::model_server::{InferenceRequest, InferenceResponse, ModelInfo, ModelServer};
}
