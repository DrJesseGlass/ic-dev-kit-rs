//! Type-safe stable storage utilities.
//!
//! This module provides generic, type-safe wrappers for saving/loading any
//! `CandidType` to IC stable storage. Requires the `storage` feature.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::storage;
//! use std::cell::RefCell;
//!
//! // Save any CandidType
//! storage::save_candid(&registry, "config", &my_config)?;
//!
//! // Load it back
//! let config: Option<MyConfig> = storage::load_candid(&registry, "config");
//! ```
//!
//! # Setting Up a Registry
//!
//! See [STORAGE_EXAMPLES.md](https://github.com/drjesseglass/ic-dev-kit-rs/blob/main/STORAGE_EXAMPLES.md)
//! for detailed setup instructions.

#![cfg(feature = "storage")]

use candid::{CandidType, Decode, Encode};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;

/// Trait for storage backends.
///
/// Implement this trait for your storage type to use the storage utilities.
///
/// # Example
///
/// ```rust,ignore
/// impl StorageRegistry for StableBTreeMap<String, Vec<u8>, Memory> {
///     fn insert(&mut self, key: String, value: Vec<u8>) {
///         StableBTreeMap::insert(self, key, value);
///     }
///
///     fn get(&self, key: &String) -> Option<Vec<u8>> {
///         StableBTreeMap::get(self, key)
///     }
///
///     fn remove(&mut self, key: &String) -> Option<Vec<u8>> {
///         StableBTreeMap::remove(self, key)
///     }
/// }
/// ```
pub trait StorageRegistry {
    /// Insert a key-value pair.
    fn insert(&mut self, key: String, value: Vec<u8>);
    /// Get a value by key.
    fn get(&self, key: &String) -> Option<Vec<u8>>;
    /// Remove and return a value by key.
    fn remove(&mut self, key: &String) -> Option<Vec<u8>>;
}

// Implement for StableBTreeMap
impl<M> StorageRegistry for StableBTreeMap<String, Vec<u8>, M>
where
    M: ic_stable_structures::Memory,
{
    fn insert(&mut self, key: String, value: Vec<u8>) {
        StableBTreeMap::insert(self, key, value);
    }

    fn get(&self, key: &String) -> Option<Vec<u8>> {
        StableBTreeMap::get(self, key)
    }

    fn remove(&mut self, key: &String) -> Option<Vec<u8>> {
        StableBTreeMap::remove(self, key)
    }
}

/// Save any `CandidType` to storage with automatic serialization.
///
/// # Arguments
///
/// * `registry` - A `RefCell` containing your storage registry
/// * `key` - The storage key
/// * `data` - The data to save
///
/// # Example
///
/// ```rust,ignore
/// REGISTRY.with(|reg| {
///     storage::save_candid(reg, "my_key", &my_data)?;
/// });
/// ```
pub fn save_candid<T: CandidType, R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
    data: &T,
) -> Result<(), String> {
    match Encode!(data) {
        Ok(serialized_bytes) => {
            registry.borrow_mut().insert(key.to_string(), serialized_bytes);
            #[cfg(feature = "telemetry")]
            crate::telemetry::log_info(&format!("Saved data to stable storage: {}", key));
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to serialize data for key {}: {:?}", key, e);
            #[cfg(feature = "telemetry")]
            crate::telemetry::log_error(&err_msg);
            Err(err_msg)
        }
    }
}

/// Load a `CandidType` from storage with automatic deserialization.
///
/// # Type Parameters
///
/// * `T` - The type to deserialize into (must implement `CandidType` and `Deserialize`)
///
/// # Arguments
///
/// * `registry` - A `RefCell` containing your storage registry
/// * `key` - The storage key
///
/// # Returns
///
/// `Some(T)` if the key exists and deserialization succeeds, `None` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// let data: Option<MyType> = REGISTRY.with(|reg| {
///     storage::load_candid(reg, "my_key")
/// });
/// ```
pub fn load_candid<T, R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> Option<T>
where
    T: for<'de> candid::Deserialize<'de> + CandidType,
{
    registry.borrow().get(&key.to_string()).and_then(|serialized_bytes| {
        match Decode!(&serialized_bytes, T) {
            Ok(data) => {
                #[cfg(feature = "telemetry")]
                crate::telemetry::log_info(&format!("Loaded data from stable storage: {}", key));
                Some(data)
            }
            Err(_e) => {
                #[cfg(feature = "telemetry")]
                crate::telemetry::log_error(&format!(
                    "Failed to deserialize data for key {}: {:?}",
                    key, _e
                ));
                None
            }
        }
    })
}

/// Save raw bytes to storage.
///
/// Use this for binary data that doesn't need Candid serialization.
pub fn save_bytes<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
    bytes: Vec<u8>,
) {
    #[cfg(feature = "telemetry")]
    let size = bytes.len();

    registry.borrow_mut().insert(key.to_string(), bytes);

    #[cfg(feature = "telemetry")]
    crate::telemetry::log_info(&format!("Saved {} bytes to stable storage: {}", size, key));
}

/// Load raw bytes from storage.
pub fn load_bytes<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> Option<Vec<u8>> {
    registry.borrow().get(&key.to_string())
}

/// Delete an entry from storage.
///
/// # Returns
///
/// `true` if the key existed and was removed.
pub fn delete<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> bool {
    let removed = registry.borrow_mut().remove(&key.to_string()).is_some();

    if removed {
        #[cfg(feature = "telemetry")]
        crate::telemetry::log_info(&format!("Deleted from stable storage: {}", key));
    }

    removed
}

/// Check if a key exists in storage.
pub fn exists<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> bool {
    registry.borrow().get(&key.to_string()).is_some()
}

/// Get the size of stored data in bytes.
///
/// # Returns
///
/// `Some(size)` if the key exists, `None` otherwise.
pub fn size<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> Option<usize> {
    registry.borrow().get(&key.to_string()).map(|bytes| bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Simple test registry
    struct TestRegistry {
        map: HashMap<String, Vec<u8>>,
    }

    impl StorageRegistry for TestRegistry {
        fn insert(&mut self, key: String, value: Vec<u8>) {
            self.map.insert(key, value);
        }

        fn get(&self, key: &String) -> Option<Vec<u8>> {
            self.map.get(key).cloned()
        }

        fn remove(&mut self, key: &String) -> Option<Vec<u8>> {
            self.map.remove(key)
        }
    }

    #[test]
    fn test_save_load_bytes() {
        let registry = RefCell::new(TestRegistry {
            map: HashMap::new(),
        });

        save_bytes(&registry, "test", vec![1, 2, 3]);
        let loaded = load_bytes(&registry, "test");

        assert_eq!(loaded, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_exists() {
        let registry = RefCell::new(TestRegistry {
            map: HashMap::new(),
        });

        assert!(!exists(&registry, "test"));
        save_bytes(&registry, "test", vec![1, 2, 3]);
        assert!(exists(&registry, "test"));
    }
}