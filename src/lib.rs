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
//! | `candle` | ML model infrastructure | `candle-core`, `candle-nn` |
//! | `text-generation` | LLM text generation | `candle`, `tokenizers` |
//!
//! ## Modules
//!
//! - [`auth`] - Principal-based authorization with guard functions
//! - [`http`] - HTTP request/response types and routing
//! - [`large_objects`] - Chunked uploads for large files
//! - [`intercanister`] - Inter-canister call wrappers with logging
//! - [`storage`] - Type-safe stable storage (requires `storage` feature)
//! - [`telemetry`] - Canistergeek integration (requires `telemetry` feature)
//! - [`candle`] - ML model traits (requires `candle` feature)
//! - [`text_generation`] - LLM generation (requires `text-generation` feature)


pub mod auth;
pub mod http;
pub mod large_objects;
pub mod intercanister;

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

pub use candid::Principal;

/// Prelude module
pub mod prelude {
    pub use crate::auth::{self, AuthError, AuthResult};
    pub use crate::http::{
        self, HttpError, HttpMethod, HttpRequest, HttpResponse, HttpResult, StreamingCallback,
        StreamingCallbackHttpResponse, StreamingCallbackToken, StreamingStrategy,
    };
    pub use crate::large_objects;
    pub use crate::intercanister;
    pub use candid::Principal;

    #[cfg(feature = "telemetry")]
    pub use crate::telemetry::{self, TelemetryError, TelemetryResult};

    #[cfg(feature = "storage")]
    pub use crate::storage::{self, StorageRegistry};

    #[cfg(feature = "candle")]
    pub use crate::candle::{self, CandleModel, ModelMetadata, ModelManager, gguf};

    #[cfg(feature = "text-generation")]
    pub use crate::text_generation::{
        self, AutoregressiveModel, GenerationConfig,
        TokenizerHandle, GenerationResponse, StopReason,
        generate_autoregressive, format_generation_stats, tokenizers,
    };

    #[cfg(all(feature = "text-generation", feature = "storage"))]
    pub use crate::model_server::{ModelServer, EmptyResult, InferenceRequest, InferenceResponse, ModelInfo};
}
