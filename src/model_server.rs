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

use crate::candle::*;
use crate::storage::StorageRegistry;
use crate::text_generation::*;
use candid::CandidType;
use serde::Deserialize;
use std::cell::RefCell;

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
        self.model
            .borrow()
            .as_ref()
            .map(|m| m.generated_token_count())
            .unwrap_or(0)
    }

    /// Get model metadata.
    pub fn metadata(&self) -> Option<ModelMetadata> {
        self.model.borrow().as_ref().map(|m| m.metadata())
    }
}

impl<M: AutoregressiveModel> Default for ModelServer<M> {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Response Types
// ═══════════════════════════════════════════════════════════════

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

impl InferenceResponse {
    /// A failed inference: empty output with `success: false` and the error.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            generated_text: String::new(),
            tokens_generated: 0,
            instructions_used: 0,
            success: false,
            error: Some(error.into()),
        }
    }
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
/// - `generate` - Run inference (guarded by `generate_guard`; public if omitted)
/// - `reset_generation` - Reset model state (admin only)
/// - `is_model_loaded` - Check if model is ready (public)
/// - `get_model_info` - Get model information (public)
///
/// Admin endpoints are guarded by
/// [`auth::is_authorized`](crate::auth::is_authorized) via a locally defined
/// `__model_admin_guard` function, so the macro works regardless of how the
/// crate is renamed in `Cargo.toml`.
///
/// # Arguments
///
/// * `server` - The thread-local ModelServer instance
/// * `registry` - The storage registry
/// * `weights_key` - Storage key for model weights
/// * `tokenizer_key` - Storage key for tokenizer
/// * `get_tokenizer` - Function to extract tokenizer from model
/// * `generate_guard` - Optional guard function name (as a string) for the
///   `generate` endpoint. **Omitting it makes inference public**: any caller
///   can then burn up to the per-message instruction limit on your cycles.
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
///     get_tokenizer: |model| Box::new(model.get_tokenizer()),
///     generate_guard: "is_authorized"
/// );
/// ```
#[macro_export]
macro_rules! generate_model_endpoints {
    (
        server: $server:expr,
        registry: $registry:expr,
        weights_key: $weights_key:expr,
        tokenizer_key: $tokenizer_key:expr,
        get_tokenizer: $get_tokenizer:expr $(,)?
    ) => {
        fn __allow_all_generate() -> Result<(), String> {
            Ok(())
        }

        $crate::generate_model_endpoints!(
            server: $server,
            registry: $registry,
            weights_key: $weights_key,
            tokenizer_key: $tokenizer_key,
            get_tokenizer: $get_tokenizer,
            generate_guard: "__allow_all_generate"
        );
    };

    (
        server: $server:expr,
        registry: $registry:expr,
        weights_key: $weights_key:expr,
        tokenizer_key: $tokenizer_key:expr,
        get_tokenizer: $get_tokenizer:expr,
        generate_guard: $generate_guard:expr $(,)?
    ) => {
        fn __model_admin_guard() -> Result<(), String> {
            $crate::auth::is_authorized()
        }

        // `ic_cdk::export_candid!` re-parses stringified endpoint signatures
        // and cannot parse `$crate`, so the request/response types are bound
        // to local aliases first. Invoke `export_candid!` in this module.
        type __ModelInferenceRequest = $crate::model_server::InferenceRequest;
        type __ModelInferenceResponse = $crate::model_server::InferenceResponse;
        type __ModelInfo = $crate::model_server::ModelInfo;

        #[ic_cdk::update(guard = "__model_admin_guard")]
        pub fn setup_model() -> Result<(), String> {
            $crate::__private::collect_metrics();

            let result = $server.with(|s| {
                $registry.with(|r| {
                    s.setup_from_storage(r, $weights_key, $tokenizer_key, $get_tokenizer)
                })
            });

            match &result {
                Ok(()) => $crate::__private::log_info("Model loaded"),
                Err(e) => $crate::__private::log_error(format!("Load failed: {}", e)),
            }

            result
        }

        #[ic_cdk::update(guard = $generate_guard)]
        pub fn generate(request: __ModelInferenceRequest) -> __ModelInferenceResponse {
            $crate::__private::collect_metrics();

            let config = request.config.unwrap_or_default();

            $server.with(|s| match s.generate(request.prompt, &config) {
                Ok(response) => response.into(),
                Err(e) => {
                    $crate::__private::log_error(format!("Generation failed: {}", e));
                    __ModelInferenceResponse::failure(e)
                }
            })
        }

        #[ic_cdk::update(guard = "__model_admin_guard")]
        pub fn reset_generation() -> Result<(), String> {
            $server.with(|s| s.reset())
        }

        #[ic_cdk::query]
        pub fn is_model_loaded() -> bool {
            $server.with(|s| s.is_loaded())
        }

        #[ic_cdk::query]
        pub fn get_model_info() -> __ModelInfo {
            $server.with(|s| __ModelInfo {
                loaded: s.is_loaded(),
                current_tokens: s.token_count(),
                metadata: s.metadata(),
            })
        }
    };
}

#[cfg(test)]
mod tests {
    //! Expands `generate_model_endpoints!` so its body is type-checked in CI.
    //! Endpoints that reach `ic0` (`setup_model`, `generate`) are not called
    //! natively; the rest are exercised.
    use super::*;
    use crate::candle::{CandleModel, ModelMetadata};
    use std::collections::HashMap;

    struct DummyModel {
        tokens: usize,
    }

    impl CandleModel for DummyModel {
        fn load(_weights: Vec<u8>, _config: Option<Vec<u8>>) -> Result<Self, String> {
            Ok(Self { tokens: 0 })
        }

        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "dummy".to_string(),
                version: "0".to_string(),
                architecture: "test".to_string(),
                parameters: 0,
                context_length: None,
            }
        }

        fn reset(&mut self) {
            self.tokens = 0;
        }
    }

    impl AutoregressiveModel for DummyModel {
        fn init_generation(
            &mut self,
            _prompt: String,
            _tokenizer: &dyn TokenizerHandle,
            _config: &GenerationConfig,
        ) -> Result<String, String> {
            self.tokens = 1;
            Ok("a".to_string())
        }

        fn generate_next_token(
            &mut self,
            _tokenizer: &dyn TokenizerHandle,
        ) -> Result<String, String> {
            self.tokens += 1;
            Ok("b".to_string())
        }

        fn is_generation_complete(&self) -> bool {
            false
        }

        fn generated_token_count(&self) -> usize {
            self.tokens
        }
    }

    struct DummyTokenizer;

    impl TokenizerHandle for DummyTokenizer {
        fn encode(&self, _text: &str) -> Result<Vec<u32>, String> {
            Ok(Vec::new())
        }

        fn decode(&self, _tokens: &[u32]) -> Result<String, String> {
            Ok(String::new())
        }

        fn vocab_size(&self) -> usize {
            0
        }
    }

    #[derive(Default)]
    struct MapRegistry(HashMap<String, Vec<u8>>);

    impl StorageRegistry for MapRegistry {
        fn insert(&mut self, key: String, value: Vec<u8>) {
            self.0.insert(key, value);
        }

        fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.0.get(key).cloned()
        }

        fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
            self.0.remove(key)
        }
    }

    thread_local! {
        static SERVER: ModelServer<DummyModel> = const { ModelServer::new() };
        static REGISTRY: RefCell<MapRegistry> = RefCell::new(MapRegistry::default());
    }

    fn generate_guard() -> Result<(), String> {
        Ok(())
    }

    // Exercises the explicit-guard arm; the no-guard arm delegates to it.
    crate::generate_model_endpoints!(
        server: SERVER,
        registry: REGISTRY,
        weights_key: "weights",
        tokenizer_key: "tokenizer",
        get_tokenizer: |_model| Box::new(DummyTokenizer),
        generate_guard: "generate_guard",
    );

    #[test]
    fn generated_endpoints_expand_and_report_unloaded_state() {
        assert!(!is_model_loaded());
        let info = get_model_info();
        assert!(!info.loaded);
        assert_eq!(info.current_tokens, 0);
        assert!(info.metadata.is_none());
        assert_eq!(reset_generation().unwrap_err(), "Model not initialized");
        assert!(generate_guard().is_ok());
        // Auth is never initialized in this test thread, so the admin guard
        // rejects without trapping.
        assert!(__model_admin_guard().is_err());
    }

    #[test]
    fn failure_response_is_empty_and_unsuccessful() {
        let resp = InferenceResponse::failure("boom");
        assert!(!resp.success);
        assert_eq!(resp.error.as_deref(), Some("boom"));
        assert!(resp.generated_text.is_empty());
        assert_eq!(resp.tokens_generated, 0);
    }
}
