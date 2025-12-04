//! Text generation features for Large Language Models.
//!
//! This module provides traits and utilities specifically for autoregressive
//! text generation with LLMs like GPT, Llama, Qwen, etc.
//!
//! Requires the `text-generation` feature.
//!
//! # Example
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::text_generation::*;
//!
//! let response = generate_autoregressive(
//!     &mut my_llm,
//!     "Hello, world!".to_string(),
//!     &tokenizer,
//!     &GenerationConfig::default()
//! )?;
//!
//! println!("Generated: {}", response.text);
//! println!("{}", format_generation_stats(&response));
//! ```

#![cfg(feature = "text-generation")]

use candid::CandidType;
use serde::Deserialize;
use crate::candle::CandleModel;

// ═══════════════════════════════════════════════════════════════
//  Autoregressive Model Traits (for LLMs)
// ═══════════════════════════════════════════════════════════════

/// Trait for autoregressive text generation models (LLMs).
///
/// Extend [`CandleModel`] with text generation capabilities.
/// Implement this for models that generate text token-by-token.
///
/// # Example
///
/// ```rust,ignore
/// impl AutoregressiveModel for MyLlama {
///     fn init_generation(
///         &mut self,
///         prompt: String,
///         tokenizer: &dyn TokenizerHandle,
///         config: &GenerationConfig,
///     ) -> Result<String, String> {
///         // Tokenize prompt, generate first token
///         // ...
///     }
///
///     fn generate_next_token(
///         &mut self,
///         tokenizer: &dyn TokenizerHandle,
///     ) -> Result<String, String> {
///         // Generate next token
///         // ...
///     }
///
///     fn is_generation_complete(&self) -> bool {
///         self.last_token == self.eos_token
///     }
///
///     fn generated_token_count(&self) -> usize {
///         self.token_count
///     }
/// }
/// ```
pub trait AutoregressiveModel: CandleModel {
    /// Initialize generation with a prompt and tokenizer.
    ///
    /// This should:
    /// 1. Clear previous generation state
    /// 2. Tokenize the prompt
    /// 3. Generate the first token
    ///
    /// # Returns
    ///
    /// The first generated token as text.
    fn init_generation(
        &mut self,
        prompt: String,
        tokenizer: &dyn TokenizerHandle,
        config: &GenerationConfig,
    ) -> Result<String, String>;

    /// Generate the next token in the sequence.
    ///
    /// # Returns
    ///
    /// The next token as text.
    fn generate_next_token(
        &mut self,
        tokenizer: &dyn TokenizerHandle,
    ) -> Result<String, String>;

    /// Check if generation is complete (EOS reached).
    fn is_generation_complete(&self) -> bool;

    /// Get current token count in generation.
    fn generated_token_count(&self) -> usize;
}

/// Handle to a tokenizer.
///
/// Abstracts the tokenizer implementation so we can support different
/// tokenizer types (tokenizers, sentencepiece, etc.).
pub trait TokenizerHandle {
    /// Encode text to token IDs.
    fn encode(&self, text: &str) -> Result<Vec<u32>, String>;
    /// Decode token IDs to text.
    fn decode(&self, tokens: &[u32]) -> Result<String, String>;
    /// Get the vocabulary size.
    fn vocab_size(&self) -> usize;
}

/// Configuration for text generation.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct GenerationConfig {
    /// Temperature for sampling (higher = more random). Default: 0.7
    pub temperature: f64,
    /// Top-p (nucleus) sampling threshold. Default: 0.9
    pub top_p: f64,
    /// Top-k sampling (None = disabled). Default: None
    pub top_k: Option<u32>,
    /// Penalty for repeated tokens. Default: 1.1
    pub repeat_penalty: f32,
    /// Number of recent tokens to consider for repeat penalty. Default: 64
    pub repeat_last_n: usize,
    /// Random seed for reproducibility. Default: 42
    pub seed: u64,
    /// Maximum tokens to generate. Default: 100
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

/// Generate text using any AutoregressiveModel implementation.
///
/// This is a generic function that works with any model implementing
/// the [`AutoregressiveModel`] trait. It handles:
/// - Instruction limit monitoring (IC-specific, 30B limit)
/// - Token limit enforcement
/// - EOS detection
/// - Error handling
///
/// # Arguments
///
/// * `model` - The model to generate with
/// * `prompt` - The input prompt
/// * `tokenizer` - The tokenizer handle
/// * `config` - Generation configuration
///
/// # Example
///
/// ```rust,ignore
/// let response = generate_autoregressive(
///     &mut my_llm,
///     "Once upon a time".to_string(),
///     &tokenizer,
///     &GenerationConfig {
///         max_tokens: 200,
///         temperature: 0.8,
///         ..Default::default()
///     }
/// )?;
/// ```
pub fn generate_autoregressive<T: AutoregressiveModel>(
    model: &mut T,
    prompt: String,
    tokenizer: &dyn TokenizerHandle,
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

/// Response from text generation.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct GenerationResponse {
    /// The generated text.
    pub text: String,
    /// Number of tokens generated.
    pub tokens_generated: usize,
    /// IC instructions used.
    pub instructions_used: u64,
    /// Why generation stopped.
    pub stopped_reason: StopReason,
}

/// Reason why generation stopped.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum StopReason {
    /// Generation completed with EOS token.
    EndOfSequence,
    /// Hit max token limit.
    MaxTokens,
    /// Hit IC instruction limit (30B).
    InstructionLimit,
    /// An error occurred.
    Error(String),
}

// ═══════════════════════════════════════════════════════════════
//  Utility Functions
// ═══════════════════════════════════════════════════════════════

/// Format generation statistics as a human-readable string.
///
/// # Example
///
/// ```rust,ignore
/// println!("{}", format_generation_stats(&response));
/// // Output: "Generated 50 tokens using 1234567890 instructions (completed)"
/// ```
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
//  Tokenizer Helpers
// ═══════════════════════════════════════════════════════════════

/// Helpers for working with tokenizers.
pub mod tokenizers {
    use tokenizers::Tokenizer;

    /// Find the EOS token from common names.
    ///
    /// Checks for common EOS token names in order:
    /// - `<|endoftext|>` (GPT-style)
    /// - `<|im_end|>` (ChatML-style)
    /// - `</s>` (Llama-style)
    /// - `<eos>` (Generic)
    ///
    /// Returns 0 if no known EOS token is found.
    pub fn find_eos_token(tokenizer: &Tokenizer) -> u32 {
        let vocab = tokenizer.get_vocab(true);

        vocab.get("<|endoftext|>")
            .or_else(|| vocab.get("<|im_end|>"))
            .or_else(|| vocab.get("</s>"))
            .or_else(|| vocab.get("<eos>"))
            .copied()
            .unwrap_or(0)
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
}