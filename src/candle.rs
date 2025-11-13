//! Generic infrastructure for Candle-based models

#![cfg(feature = "candle")]

use candid::CandidType;
use serde::Deserialize;

// ═══════════════════════════════════════════════════════════════
//  Generic Model Traits (for ALL model types)
// ═══════════════════════════════════════════════════════════════

/// Trait for models that can be loaded from bytes
///
/// This is the base trait for all Candle models, regardless of type.
pub trait CandleModel: Sized {
    /// Load model from raw bytes
    ///
    /// # Arguments
    /// * `weights` - Model weights (typically GGUF or safetensors format)
    /// * `config` - Optional configuration data
    ///
    /// # Returns
    /// * `Result<Self, String>` - Loaded model or error
    fn load(weights: Vec<u8>, config: Option<Vec<u8>>) -> Result<Self, String>;

    /// Get model metadata
    fn metadata(&self) -> ModelMetadata;

    /// Reset model state (clear caches, etc.)
    fn reset(&mut self);
}

/// Model metadata
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ModelMetadata {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub parameters: u64,
    pub context_length: Option<usize>,
}

// ═══════════════════════════════════════════════════════════════
//  Autoregressive Model Traits (for LLMs/Text Generation)
// ═══════════════════════════════════════════════════════════════

/// Trait for autoregressive text generation models (LLMs)
///
/// This trait is specifically for models that generate text token-by-token,
/// such as GPT, Llama, Qwen, etc.
pub trait AutoregressiveModel: CandleModel {
    /// Initialize generation with a prompt and tokenizer
    ///
    /// This should:
    /// - Clear previous generation state
    /// - Load the tokenizer
    /// - Tokenize the prompt
    /// - Generate the first token
    ///
    /// # Returns
    /// * First generated token as text
    fn init_generation(
        &mut self,
        prompt: String,
        tokenizer: &TokenizerHandle,
        config: &GenerationConfig,
    ) -> Result<String, String>;

    /// Generate the next token in the sequence
    ///
    /// # Returns
    /// * Next token as text
    fn generate_next_token(
        &mut self,
        tokenizer: &TokenizerHandle,
    ) -> Result<String, String>;

    /// Check if generation is complete (EOS reached)
    fn is_generation_complete(&self) -> bool;

    /// Get current token count in generation
    fn generated_token_count(&self) -> usize;
}

/// Handle to a tokenizer
///
/// This abstracts the tokenizer so we can support different tokenizer types
pub trait TokenizerHandle {
    fn encode(&self, text: &str) -> Result<Vec<u32>, String>;
    fn decode(&self, tokens: &[u32]) -> Result<String, String>;
    fn vocab_size(&self) -> usize;
}

/// Generation configuration for autoregressive models
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct GenerationConfig {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: Option<u32>,
    pub repeat_penalty: f32,
    pub repeat_last_n: usize,
    pub seed: u64,
    pub max_tokens: usize,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: None,
            repeat_penalty: 1.1,
            repeat_last_n: 64,
            seed: 42,
            max_tokens: 100,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Generic Autoregressive Generation Function
// ═══════════════════════════════════════════════════════════════

/// Generate text using any AutoregressiveModel implementation
///
/// This is a generic function that works with any model implementing
/// the AutoregressiveModel trait. It handles:
/// - Instruction limit monitoring (IC-specific)
/// - Token limit enforcement
/// - EOS detection
/// - Error handling
///
/// # Example
/// ```rust,ignore
/// let response = generate_autoregressive(
///     &mut my_llm,
///     "Hello, world!",
///     &tokenizer,
///     &GenerationConfig::default()
/// )?;
/// ```
pub fn generate_autoregressive<T: AutoregressiveModel>(
    model: &mut T,
    prompt: String,
    tokenizer: &impl TokenizerHandle,
    config: &GenerationConfig,
) -> Result<GenerationResponse, String> {
    let start_instructions = ic_cdk::api::performance_counter(0);

    // Initialize with prompt and generate first token
    let first_token = model.init_generation(prompt, tokenizer, config)?;
    let mut generated_text = first_token;

    // Generate remaining tokens
    for _ in 1..config.max_tokens {
        // Check if we hit EOS
        if model.is_generation_complete() {
            let instructions_used = ic_cdk::api::performance_counter(0) - start_instructions;
            return Ok(GenerationResponse {
                text: generated_text,
                tokens_generated: model.generated_token_count(),
                instructions_used,
                stopped_reason: StopReason::EndOfSequence,
            });
        }

        // Check instruction limit (30B for IC)
        let instructions_so_far = ic_cdk::api::performance_counter(0) - start_instructions;
        if instructions_so_far > 30_000_000_000 {
            return Ok(GenerationResponse {
                text: generated_text,
                tokens_generated: model.generated_token_count(),
                instructions_used: instructions_so_far,
                stopped_reason: StopReason::InstructionLimit,
            });
        }

        // Generate next token
        let token_text = model.generate_next_token(tokenizer)?;
        generated_text.push_str(&token_text);
    }

    // Hit max tokens
    let instructions_used = ic_cdk::api::performance_counter(0) - start_instructions;
    Ok(GenerationResponse {
        text: generated_text,
        tokens_generated: model.generated_token_count(),
        instructions_used,
        stopped_reason: StopReason::MaxTokens,
    })
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct GenerationResponse {
    pub text: String,
    pub tokens_generated: usize,
    pub instructions_used: u64,
    pub stopped_reason: StopReason,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum StopReason {
    /// Generation completed with EOS token
    EndOfSequence,
    /// Hit max token limit
    MaxTokens,
    /// Hit instruction limit (IC-specific)
    InstructionLimit,
    /// Error occurred
    Error(String),
}

// ═══════════════════════════════════════════════════════════════
//  Model Manager (for managing multiple models)
// ═══════════════════════════════════════════════════════════════

use std::collections::HashMap;

/// Manager for multiple models
///
/// This allows you to load and manage multiple models in a single canister,
/// switching between them as needed.
pub struct ModelManager<T> {
    models: HashMap<String, T>,
    active_model: Option<String>,
}

impl<T> ModelManager<T> {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            active_model: None,
        }
    }

    /// Register a model with a name
    pub fn register(&mut self, name: String, model: T) {
        self.models.insert(name.clone(), model);
        if self.active_model.is_none() {
            self.active_model = Some(name);
        }
    }

    /// Set the active model
    pub fn set_active(&mut self, name: &str) -> Result<(), String> {
        if self.models.contains_key(name) {
            self.active_model = Some(name.to_string());
            Ok(())
        } else {
            Err(format!("Model '{}' not found", name))
        }
    }

    /// Get the active model
    pub fn active(&self) -> Option<&T> {
        self.active_model.as_ref()
            .and_then(|name| self.models.get(name))
    }

    /// Get a mutable reference to the active model
    pub fn active_mut(&mut self) -> Option<&mut T> {
        let name = self.active_model.clone()?;
        self.models.get_mut(&name)
    }

    /// Get a model by name
    pub fn get(&self, name: &str) -> Option<&T> {
        self.models.get(name)
    }

    /// Get a mutable reference to a model by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut T> {
        self.models.get_mut(name)
    }

    /// List all registered models
    pub fn list(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }

    /// Remove a model
    pub fn remove(&mut self, name: &str) -> Option<T> {
        let model = self.models.remove(name);
        if Some(name) == self.active_model.as_deref() {
            self.active_model = None;
        }
        model
    }

    /// Get active model name
    pub fn active_name(&self) -> Option<&str> {
        self.active_model.as_deref()
    }
}

impl<T> Default for ModelManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Utility Functions
// ═══════════════════════════════════════════════════════════════

/// Format generation statistics
pub fn format_generation_stats(response: &GenerationResponse) -> String {
    format!(
        "Generated {} tokens using {} instructions ({})",
        response.tokens_generated,
        response.instructions_used,
        match &response.stopped_reason {
            StopReason::EndOfSequence => "completed",
            StopReason::MaxTokens => "max tokens reached",
            StopReason::InstructionLimit => "instruction limit reached",
            StopReason::Error(e) => return format!("error: {}", e),
        }
    )
}

// ═══════════════════════════════════════════════════════════════
//  GGUF and Tokenizer Helpers
// ═══════════════════════════════════════════════════════════════

pub mod gguf {
    use candle_core::Device;
    use candle_core::quantized::gguf_file;
    use std::io::Cursor;

    /// Load GGUF content from bytes
    pub fn load_content(weights: Vec<u8>) -> Result<(gguf_file::Content, Cursor<Vec<u8>>), String> {
        let mut cursor = Cursor::new(weights);
        let content = gguf_file::Content::read(&mut cursor)
            .map_err(|e| format!("Failed to read GGUF: {}", e))?;
        Ok((content, cursor))
    }

    /// Get CPU device (helper)
    pub fn cpu_device() -> Device {
        Device::Cpu
    }

    /// Tokenizer helper functions
    pub mod tokenizers {
        use tokenizers::Tokenizer;

        /// Find EOS token from common names
        pub fn find_eos_token(tokenizer: &Tokenizer) -> u32 {
            tokenizer.get_vocab(true)
                .get("<|endoftext|>")
                .or_else(|| tokenizer.get_vocab(true).get("<|im_end|>"))
                .or_else(|| tokenizer.get_vocab(true).get("</s>"))
                .or_else(|| tokenizer.get_vocab(true).get("<eos>"))
                .copied()
                .unwrap_or(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_config_default() {
        let config = GenerationConfig::default();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 100);
    }

    #[test]
    fn test_model_manager() {
        let mut manager: ModelManager<String> = ModelManager::new();

        manager.register("model1".to_string(), "data1".to_string());
        manager.register("model2".to_string(), "data2".to_string());

        assert_eq!(manager.list().len(), 2);
        assert_eq!(manager.active(), Some(&"data1".to_string()));
        assert_eq!(manager.active_name(), Some("model1"));

        manager.set_active("model2").unwrap();
        assert_eq!(manager.active(), Some(&"data2".to_string()));
        assert_eq!(manager.active_name(), Some("model2"));

        manager.remove("model1");
        assert_eq!(manager.list().len(), 1);
    }
}