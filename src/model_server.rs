//! Generic model server for LLM inference.
//!
//! Provides a ready-to-use server that combines storage (for model weights)
//! with text generation. Requires both `text-generation` and `storage` features.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::model_server::ModelServer;
//! use std::cell::RefCell;
//!
//! thread_local! {
//!     static SERVER: ModelServer<MyLlm> = ModelServer::new();
//! }
//!
//! // Use the macro to generate all endpoints
//! ic_dev_kit_rs::generate_model_endpoints!(
//!     server: SERVER,
//!     registry: REGISTRIES,
//!     weights_key: "model_weights",
//!     tokenizer_key: "tokenizer",
//!     get_tokenizer: |model| Box::new(model.get_tokenizer())
//! );
//! ```

#![cfg(all(feature = "text-generation", feature = "storage"))]

use std::cell::RefCell;
use candid::CandidType;
use serde::Deserialize;
use crate::candle::*;
use crate::text_generation::*;
use crate::storage::StorageRegistry;

/// Generic model server for LLM inference.
///
/// Manages model loading from storage and inference. Thread-safe for IC.
///
/// # Type Parameters
///
/// * `M` - The model type (must implement [`AutoregressiveModel`])
pub struct ModelServer<M: AutoregressiveModel> {
    model: RefCell<Option<M>>,
    tokenizer: RefCell<Option<Box<dyn TokenizerHandle>>>,
}

impl<M: AutoregressiveModel> ModelServer<M> {
    /// Create a new uninitialized model server.
    pub const fn new() -> Self {
        Self {
            model: RefCell::new(None),
            tokenizer: RefCell::new(None),
        }
    }

    /// Set up the model from storage.
    ///
    /// Loads model weights and tokenizer from the storage registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - The storage registry containing model data
    /// * `weights_key` - Storage key for model weights
    /// * `tokenizer_key` - Storage key for tokenizer data
    /// * `get_tokenizer` - Function to extract tokenizer from loaded model
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// server.setup_from_storage(
    ///     &registry,
    ///     "model_weights",
    ///     "tokenizer",
    ///     |model| Box::new(model.tokenizer().clone())
    /// )?;
    /// ```
    pub fn setup_from_storage<R: StorageRegistry>(
        &self,
        registry: &RefCell<R>,
        weights_key: &str,
        tokenizer_key: &str,
        get_tokenizer: impl FnOnce(&M) -> Box<dyn TokenizerHandle>,
    ) -> Result<(), String> {
        let weights = crate::storage::load_bytes(registry, weights_key)
            .ok_or(format!("Weights not found: {}", weights_key))?;

        let tokenizer_bytes = crate::storage::load_bytes(registry, tokenizer_key)
            .ok_or(format!("Tokenizer not found: {}", tokenizer_key))?;

        let model = M::load(weights, Some(tokenizer_bytes))?;
        let tokenizer = get_tokenizer(&model);

        *self.model.borrow_mut() = Some(model);
        *self.tokenizer.borrow_mut() = Some(tokenizer);

        Ok(())
    }

    /// Generate text from a prompt.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The input prompt
    /// * `config` - Generation configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the model is not initialized.
    pub fn generate(
        &self,
        prompt: String,
        config: &GenerationConfig,
    ) -> Result<GenerationResponse, String> {
        let mut model = self.model.borrow_mut();
        let tokenizer = self.tokenizer.borrow();

        let model = model.as_mut().ok_or("Model not initialized")?;
        let tokenizer = tokenizer.as_ref().ok_or("Tokenizer not initialized")?;

        generate_autoregressive(model, prompt, tokenizer.as_ref(), config)
    }

    /// Reset the model's generation state.
    ///
    /// Clears KV cache and other generation state.
    pub fn reset(&self) -> Result<(), String> {
        let mut model = self.model.borrow_mut();
        model.as_mut().ok_or("Model not initialized")?.reset();
        Ok(())
    }

    /// Check if the model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.model.borrow().is_some()
    }

    /// Get the current token count (for multi-turn generation).
    pub fn token_count(&self) -> usize {
        self.model.borrow().as_ref().map(|m| m.generated_token_count()).unwrap_or(0)
    }

    /// Get model metadata.
    pub fn metadata(&self) -> Option<ModelMetadata> {
        self.model.borrow().as_ref().map(|m| m.metadata())
    }
}

// ═══════════════════════════════════════════════════════════════
//  Response Types
// ═══════════════════════════════════════════════════════════════

/// Simple result type for endpoints with no return value.
#[derive(CandidType, Deserialize)]
pub enum EmptyResult {
    /// Success.
    Ok,
    /// Error with message.
    Err(String),
}

/// Request for inference.
#[derive(CandidType, Deserialize)]
pub struct InferenceRequest {
    /// The input prompt.
    pub prompt: String,
    /// Optional generation config (uses defaults if None).
    pub config: Option<GenerationConfig>,
}

/// Response from inference.
#[derive(CandidType, Deserialize)]
pub struct InferenceResponse {
    /// The generated text.
    pub generated_text: String,
    /// Number of tokens generated.
    pub tokens_generated: usize,
    /// IC instructions used.
    pub instructions_used: u64,
    /// Whether generation succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

impl From<GenerationResponse> for InferenceResponse {
    fn from(resp: GenerationResponse) -> Self {
        Self {
            generated_text: resp.text,
            tokens_generated: resp.tokens_generated,
            instructions_used: resp.instructions_used,
            success: true,
            error: None,
        }
    }
}

/// Model information for status queries.
#[derive(CandidType, Deserialize)]
pub struct ModelInfo {
    /// Whether the model is loaded.
    pub loaded: bool,
    /// Current token count in context.
    pub current_tokens: usize,
    /// Model metadata (if loaded).
    pub metadata: Option<ModelMetadata>,
}

// ═══════════════════════════════════════════════════════════════
//  Macro for Generating Endpoints
// ═══════════════════════════════════════════════════════════════

/// Generate all IC endpoints for a model server.
///
/// This macro creates the following endpoints:
/// - `setup_model` - Load model from storage (admin only)
/// - `generate` - Run inference (public)
/// - `reset_generation` - Reset model state (admin only)
/// - `is_model_loaded` - Check if model is ready (public)
/// - `get_model_info` - Get model information (public)
///
/// # Arguments
///
/// * `server` - The thread-local ModelServer instance
/// * `registry` - The storage registry
/// * `weights_key` - Storage key for model weights
/// * `tokenizer_key` - Storage key for tokenizer
/// * `get_tokenizer` - Function to extract tokenizer from model
///
/// # Example
///
/// ```rust,ignore
/// thread_local! {
///     static SERVER: ModelServer<MyLlm> = ModelServer::new();
///     static REGISTRIES: RefCell<StableBTreeMap<...>> = ...;
/// }
///
/// ic_dev_kit_rs::generate_model_endpoints!(
///     server: SERVER,
///     registry: REGISTRIES,
///     weights_key: "model_weights",
///     tokenizer_key: "tokenizer",
///     get_tokenizer: |model| Box::new(model.get_tokenizer())
/// );
/// ```
#[macro_export]
macro_rules! generate_model_endpoints {
    (
        server: $server:expr,
        registry: $registry:expr,
        weights_key: $weights_key:expr,
        tokenizer_key: $tokenizer_key:expr,
        get_tokenizer: $get_tokenizer:expr
    ) => {
        use $crate::model_server::{EmptyResult, InferenceRequest, InferenceResponse, ModelInfo};

        #[ic_cdk::update(guard = "ic_dev_kit_rs::auth::is_authorized")]
        pub fn setup_model() -> EmptyResult {
            #[cfg(feature = "telemetry")]
            $crate::telemetry::collect_metrics();

            match $server.with(|s| {
                $registry.with(|r| {
                    s.setup_from_storage(r, $weights_key, $tokenizer_key, $get_tokenizer)
                })
            }) {
                Ok(_) => {
                    #[cfg(feature = "telemetry")]
                    $crate::telemetry::log_info("Model loaded");
                    EmptyResult::Ok
                }
                Err(e) => {
                    #[cfg(feature = "telemetry")]
                    $crate::telemetry::log_error(&format!("Load failed: {}", e));
                    EmptyResult::Err(e)
                }
            }
        }

        #[ic_cdk::update]
        pub fn generate(request: InferenceRequest) -> InferenceResponse {
            #[cfg(feature = "telemetry")]
            $crate::telemetry::collect_metrics();

            let config = request.config.unwrap_or_default();

            $server.with(|s| {
                match s.generate(request.prompt, &config) {
                    Ok(response) => response.into(),
                    Err(e) => {
                        #[cfg(feature = "telemetry")]
                        $crate::telemetry::log_error(&format!("Generation failed: {}", e));
                        InferenceResponse {
                            generated_text: String::new(),
                            tokens_generated: 0,
                            instructions_used: 0,
                            success: false,
                            error: Some(e),
                        }
                    }
                }
            })
        }

        #[ic_cdk::update(guard = "ic_dev_kit_rs::auth::is_authorized")]
        pub fn reset_generation() -> EmptyResult {
            $server.with(|s| match s.reset() {
                Ok(_) => EmptyResult::Ok,
                Err(e) => EmptyResult::Err(e),
            })
        }

        #[ic_cdk::query]
        pub fn is_model_loaded() -> bool {
            $server.with(|s| s.is_loaded())
        }

        #[ic_cdk::query]
        pub fn get_model_info() -> ModelInfo {
            $server.with(|s| ModelInfo {
                loaded: s.is_loaded(),
                current_tokens: s.token_count(),
                metadata: s.metadata(),
            })
        }
    };
}