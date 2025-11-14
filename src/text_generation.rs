//! Text generation features for LLMs
//!
//! This module provides traits and utilities specifically for Large Language Models
//! and autoregressive text generation. It requires the "text-generation" feature.

#![cfg(feature = "text-generation")]

use candid::CandidType;
use serde::Deserialize;
use crate::candle::CandleModel;

// ═══════════════════════════════════════════════════════════════
//  Autoregressive Model Traits (for LLMs)
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
        tokenizer: &dyn TokenizerHandle,
        config: &GenerationConfig,
    ) -> Result<String, String>;

    /// Generate the next token in the sequence
    ///
    /// # Returns
    /// * Next token as text
    fn generate_next_token(
        &mut self,
        tokenizer: &dyn TokenizerHandle,
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
//  Tokenizer Helpers
// ═══════════════════════════════════════════════════════════════

pub mod tokenizers {
    use tokenizers::Tokenizer;

    /// Find EOS token from common names
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