//! Generic infrastructure for Candle-based models
//!
//! This module provides core model abstractions that work for any model type:
//! - Vision models (image classification, object detection)
//! - Audio models (speech recognition, music generation)
//! - Text models (see `text_generation` module for LLM-specific features)
//! - Multimodal models
//!
//! For LLM-specific features, enable the "text-generation" feature.

#![cfg(feature = "candle")]

use candid::CandidType;
use serde::Deserialize;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
//  Generic Model Traits (for ALL model types)
// ═══════════════════════════════════════════════════════════════

/// Trait for models that can be loaded from bytes
///
/// This is the base trait for all Candle models, regardless of type.
/// Implement this for vision models, audio models, LLMs, etc.
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
//  Model Manager (for managing multiple models)
// ═══════════════════════════════════════════════════════════════

/// Manager for multiple models
///
/// This allows you to load and manage multiple models in a single canister,
/// switching between them as needed. Works with any type implementing CandleModel.
///
/// # Example
/// ```rust,ignore
/// let mut manager: ModelManager<MyVisionModel> = ModelManager::new();
/// manager.register("resnet-50".to_string(), resnet_model);
/// manager.register("yolo-v8".to_string(), yolo_model);
/// manager.set_active("resnet-50")?;
/// ```
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
//  GGUF Helpers (for all model types)
// ═══════════════════════════════════════════════════════════════

pub mod gguf {
    use candle_core::Device;
    use candle_core::quantized::gguf_file;
    use std::io::Cursor;

    /// Load GGUF content from bytes
    ///
    /// GGUF is a file format for storing quantized models efficiently.
    /// This helper loads GGUF files from bytes, useful for models stored in IC stable memory.
    pub fn load_content(weights: Vec<u8>) -> Result<(gguf_file::Content, Cursor<Vec<u8>>), String> {
        let mut cursor = Cursor::new(weights);
        let content = gguf_file::Content::read(&mut cursor)
            .map_err(|e| format!("Failed to read GGUF: {}", e))?;
        Ok((content, cursor))
    }

    /// Get CPU device (helper)
    ///
    /// Returns a Candle device configured for CPU inference.
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