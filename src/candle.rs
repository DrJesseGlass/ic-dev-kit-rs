//! Generic infrastructure for Candle-based ML models.
//!
//! This module provides core model abstractions that work for any model type:
//! - Vision models (image classification, object detection)
//! - Audio models (speech recognition, music generation)
//! - Text models (see [`text_generation`](crate::text_generation) for LLM-specific features)
//! - Multimodal models
//!
//! Requires the `candle` feature.
//!
//! # Example
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::candle::{CandleModel, ModelManager};
//!
//! // Register multiple models
//! let mut manager: ModelManager<MyModel> = ModelManager::new();
//! manager.register("model-v1".to_string(), model_v1);
//! manager.register("model-v2".to_string(), model_v2);
//!
//! // Switch between models
//! manager.set_active("model-v2")?;
//! ```

#![cfg(feature = "candle")]

use candid::CandidType;
use serde::Deserialize;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
//  Generic Model Traits (for ALL model types)
// ═══════════════════════════════════════════════════════════════

/// Trait for models that can be loaded from bytes.
///
/// This is the base trait for all Candle models, regardless of type.
/// Implement this for vision models, audio models, LLMs, etc.
///
/// # Example
///
/// ```rust,ignore
/// impl CandleModel for MyVisionModel {
///     fn load(weights: Vec<u8>, config: Option<Vec<u8>>) -> Result<Self, String> {
///         // Load GGUF weights
///         let (content, cursor) = gguf::load_content(weights)?;
///         // Build model from content...
///         Ok(Self { /* ... */ })
///     }
///
///     fn metadata(&self) -> ModelMetadata {
///         ModelMetadata {
///             name: "my-vision-model".to_string(),
///             version: "1.0".to_string(),
///             architecture: "ResNet".to_string(),
///             parameters: 25_000_000,
///             context_length: None,
///         }
///     }
///
///     fn reset(&mut self) {
///         // Clear any cached state
///     }
/// }
/// ```
pub trait CandleModel: Sized {
    /// Load model from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `weights` - Model weights (typically GGUF or safetensors format)
    /// * `config` - Optional configuration data (e.g., tokenizer for LLMs)
    fn load(weights: Vec<u8>, config: Option<Vec<u8>>) -> Result<Self, String>;

    /// Get model metadata.
    fn metadata(&self) -> ModelMetadata;

    /// Reset model state (clear caches, etc.).
    fn reset(&mut self);
}

/// Model metadata.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ModelMetadata {
    /// Model name.
    pub name: String,
    /// Model version.
    pub version: String,
    /// Architecture type (e.g., "Llama", "ResNet", "Whisper").
    pub architecture: String,
    /// Number of parameters.
    pub parameters: u64,
    /// Context length for sequence models (None for non-sequence models).
    pub context_length: Option<usize>,
}

// ═══════════════════════════════════════════════════════════════
//  Model Manager (for managing multiple models)
// ═══════════════════════════════════════════════════════════════

/// Manager for multiple models of the same type.
///
/// Allows loading and switching between multiple models in a single canister.
///
/// # Example
///
/// ```rust,ignore
/// let mut manager: ModelManager<MyVisionModel> = ModelManager::new();
///
/// // Register models
/// manager.register("resnet-50".to_string(), resnet_model);
/// manager.register("yolo-v8".to_string(), yolo_model);
///
/// // Use active model (first registered by default)
/// if let Some(model) = manager.active() {
///     let result = model.classify(&image);
/// }
///
/// // Switch active model
/// manager.set_active("yolo-v8")?;
/// ```
pub struct ModelManager<T> {
    models: HashMap<String, T>,
    active_model: Option<String>,
}

impl<T> ModelManager<T> {
    /// Create a new empty model manager.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            active_model: None,
        }
    }

    /// Register a model with a name.
    ///
    /// The first registered model becomes active by default.
    pub fn register(&mut self, name: String, model: T) {
        self.models.insert(name.clone(), model);
        if self.active_model.is_none() {
            self.active_model = Some(name);
        }
    }

    /// Set the active model by name.
    ///
    /// # Errors
    ///
    /// Returns an error if no model with the given name is registered.
    pub fn set_active(&mut self, name: &str) -> Result<(), String> {
        if self.models.contains_key(name) {
            self.active_model = Some(name.to_string());
            Ok(())
        } else {
            Err(format!("Model '{}' not found", name))
        }
    }

    /// Get a reference to the active model.
    pub fn active(&self) -> Option<&T> {
        self.active_model.as_ref()
            .and_then(|name| self.models.get(name))
    }

    /// Get a mutable reference to the active model.
    pub fn active_mut(&mut self) -> Option<&mut T> {
        let name = self.active_model.clone()?;
        self.models.get_mut(&name)
    }

    /// Get a model by name.
    pub fn get(&self, name: &str) -> Option<&T> {
        self.models.get(name)
    }

    /// Get a mutable reference to a model by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut T> {
        self.models.get_mut(name)
    }

    /// List all registered model names.
    pub fn list(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }

    /// Remove a model by name.
    ///
    /// If the removed model was active, active is set to None.
    pub fn remove(&mut self, name: &str) -> Option<T> {
        let model = self.models.remove(name);
        if Some(name) == self.active_model.as_deref() {
            self.active_model = None;
        }
        model
    }

    /// Get the name of the active model.
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
//  GGUF Helpers (for all model types)
// ═══════════════════════════════════════════════════════════════

/// Helpers for loading GGUF format models.
///
/// GGUF is a file format for storing quantized models efficiently.
pub mod gguf {
    use candle_core::Device;
    use candle_core::quantized::gguf_file;
    use std::io::Cursor;

    /// Load GGUF content from bytes.
    ///
    /// # Arguments
    ///
    /// * `weights` - Raw GGUF file bytes
    ///
    /// # Returns
    ///
    /// A tuple of (Content, Cursor) for further processing.
    pub fn load_content(weights: Vec<u8>) -> Result<(gguf_file::Content, Cursor<Vec<u8>>), String> {
        let mut cursor = Cursor::new(weights);
        let content = gguf_file::Content::read(&mut cursor)
            .map_err(|e| format!("Failed to read GGUF: {}", e))?;
        Ok((content, cursor))
    }

    /// Get CPU device.
    ///
    /// Most IC canisters will use CPU inference.
    pub fn cpu_device() -> Device {
        Device::Cpu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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