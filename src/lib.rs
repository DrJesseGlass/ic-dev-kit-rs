//! # ic-dev-kit-rs

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
    pub use crate::http::{self, HttpError, HttpRequest, HttpResponse, HttpResult, HttpMethod};
    pub use crate::large_objects;
    pub use crate::intercanister;
    pub use candid::Principal;

    #[cfg(feature = "telemetry")]
    pub use crate::telemetry::{self, TelemetryError, TelemetryResult};

    #[cfg(feature = "storage")]
    pub use crate::storage::{self, StorageRegistry};

    #[cfg(feature = "candle")]
    pub use crate::candle::{
        self, CandleModel, ModelMetadata, ModelManager, gguf,
    };

    #[cfg(feature = "text-generation")]
    pub use crate::text_generation::{
        self, AutoregressiveModel, GenerationConfig,
        TokenizerHandle, GenerationResponse, StopReason,
        generate_autoregressive, format_generation_stats, tokenizers,
    };

    #[cfg(all(feature = "text-generation", feature = "storage"))]
    pub use crate::model_server::{ModelServer, EmptyResult, InferenceRequest, InferenceResponse, ModelInfo};
}
