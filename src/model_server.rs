//! Generic model server

#![cfg(feature = "candle")]

use std::cell::RefCell;
use candid::CandidType;
use serde::Deserialize;
use crate::candle::*;
use crate::storage::StorageRegistry;

pub struct ModelServer<M: AutoregressiveModel> {
    model: RefCell<Option<M>>,
    tokenizer: RefCell<Option<Box<dyn TokenizerHandle>>>,
}

impl<M: AutoregressiveModel> ModelServer<M> {
    pub const fn new() -> Self {
        Self {
            model: RefCell::new(None),
            tokenizer: RefCell::new(None),
        }
    }

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

    pub fn reset(&self) -> Result<(), String> {
        let mut model = self.model.borrow_mut();
        model.as_mut().ok_or("Model not initialized")?.reset();
        Ok(())
    }

    pub fn is_loaded(&self) -> bool {
        self.model.borrow().is_some()
    }

    pub fn token_count(&self) -> usize {
        self.model.borrow().as_ref().map(|m| m.generated_token_count()).unwrap_or(0)
    }

    pub fn metadata(&self) -> Option<ModelMetadata> {
        self.model.borrow().as_ref().map(|m| m.metadata())
    }
}

// Response types
#[derive(CandidType, Deserialize)]
pub enum EmptyResult {
    Ok,
    Err(String),
}

#[derive(CandidType, Deserialize)]
pub struct InferenceRequest {
    pub prompt: String,
    pub config: Option<GenerationConfig>,
}

#[derive(CandidType, Deserialize)]
pub struct InferenceResponse {
    pub generated_text: String,
    pub tokens_generated: usize,
    pub instructions_used: u64,
    pub success: bool,
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

#[derive(CandidType, Deserialize)]
pub struct ModelInfo {
    pub loaded: bool,
    pub current_tokens: usize,
    pub metadata: Option<ModelMetadata>,
}

/// Macro to generate all IC endpoints for a model server
///
/// This generates: setup_model, generate, reset_generation, is_model_loaded, get_model_info
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
            $crate::telemetry::track_metrics();

            match $server.with(|s| s.setup_from_storage(&$registry, $weights_key, $tokenizer_key, $get_tokenizer)) {
                Ok(_) => {
                    $crate::telemetry::log_info("Model loaded");
                    EmptyResult::Ok
                }
                Err(e) => {
                    $crate::telemetry::log_error(&format!("Load failed: {}", e));
                    EmptyResult::Err(e)
                }
            }
        }

        #[ic_cdk::update]
        pub fn generate(request: InferenceRequest) -> InferenceResponse {
            $crate::telemetry::track_metrics();

            let config = request.config.unwrap_or_default();

            $server.with(|s| {
                match s.generate(request.prompt, &config) {
                    Ok(response) => response.into(),
                    Err(e) => {
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