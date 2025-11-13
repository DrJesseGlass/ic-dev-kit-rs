//! # ic-dev-kit-rs

pub mod auth;
pub mod http;
pub mod telemetry;
pub mod large_objects;
pub mod intercanister;

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "candle")]
pub mod candle;

#[cfg(feature = "candle")]
pub mod model_server;

pub use candid::Principal;

/// Prelude module
pub mod prelude {
    pub use crate::auth::{self, AuthError, AuthResult};
    pub use crate::http::{self, HttpError, HttpRequest, HttpResponse, HttpResult, HttpMethod};
    pub use crate::telemetry::{self, TelemetryError, TelemetryResult};
    pub use crate::large_objects;
    pub use crate::intercanister;
    pub use candid::Principal;

    #[cfg(feature = "storage")]
    pub use crate::storage::{self, StorageRegistry};

    #[cfg(feature = "candle")]
    pub use crate::candle::{
        self, CandleModel, AutoregressiveModel, GenerationConfig,
        TokenizerHandle, ModelMetadata, GenerationResponse, StopReason,
    };

    #[cfg(feature = "candle")]
    pub use crate::model_server::{ModelServer, EmptyResult, InferenceRequest, InferenceResponse, ModelInfo};
}

#[cfg(feature = "candle")]
pub use crate::generate_model_endpoints;