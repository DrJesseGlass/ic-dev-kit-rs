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
/// impl StorageRegistry for MyMap {
///     fn insert(&mut self, key: String, value: Vec<u8>) {
///         self.map.insert(key, value);
///     }
///
///     fn get(&self, key: &str) -> Option<Vec<u8>> {
///         self.map.get(key).cloned()
///     }
///
///     fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
///         self.map.remove(key)
///     }
///
///     // Override when the backend can answer without copying the value.
///     fn contains_key(&self, key: &str) -> bool {
///         self.map.contains_key(key)
///     }
/// }
/// ```
pub trait StorageRegistry {
    /// Insert a key-value pair.
    fn insert(&mut self, key: String, value: Vec<u8>);
    /// Get a value by key.
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    /// Remove and return a value by key.
    fn remove(&mut self, key: &str) -> Option<Vec<u8>>;
    /// Check whether a key exists.
    ///
    /// The default implementation calls [`get`](Self::get) and discards the
    /// value, which copies the whole entry out of storage. Override it when the
    /// backend can answer more cheaply.
    fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

// Implement for StableBTreeMap
impl<M> StorageRegistry for StableBTreeMap<String, Vec<u8>, M>
where
    M: ic_stable_structures::Memory,
{
    fn insert(&mut self, key: String, value: Vec<u8>) {
        StableBTreeMap::insert(self, key, value);
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        StableBTreeMap::get(self, &key.to_string())
    }

    fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
        StableBTreeMap::remove(self, &key.to_string())
    }

    fn contains_key(&self, key: &str) -> bool {
        StableBTreeMap::contains_key(self, &key.to_string())
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
    registry.borrow().get(key).and_then(|serialized_bytes| {
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
    registry.borrow().get(key)
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
    let removed = registry.borrow_mut().remove(key).is_some();

    if removed {
        #[cfg(feature = "telemetry")]
        crate::telemetry::log_info(&format!("Deleted from stable storage: {}", key));
    }

    removed
}

/// Check if a key exists in storage without copying the value out.
pub fn exists<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> bool {
    registry.borrow().contains_key(key)
}

/// Get the size of stored data in bytes.
///
/// This reads the whole value out of storage to measure it, so avoid calling
/// it in hot paths for large entries.
///
/// # Returns
///
/// `Some(size)` if the key exists, `None` otherwise.
pub fn size<R: StorageRegistry>(
    registry: &RefCell<R>,
    key: &str,
) -> Option<usize> {
    registry.borrow().get(key).map(|bytes| bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Simple test registry. Counts `get` calls so tests can prove that
    // existence checks do not copy values out.
    struct TestRegistry {
        map: HashMap<String, Vec<u8>>,
        gets: std::cell::Cell<usize>,
    }

    impl StorageRegistry for TestRegistry {
        fn insert(&mut self, key: String, value: Vec<u8>) {
            self.map.insert(key, value);
        }

        fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.gets.set(self.gets.get() + 1);
            self.map.get(key).cloned()
        }

        fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
            self.map.remove(key)
        }

        fn contains_key(&self, key: &str) -> bool {
            self.map.contains_key(key)
        }
    }

    fn registry() -> RefCell<TestRegistry> {
        RefCell::new(TestRegistry {
            map: HashMap::new(),
            gets: std::cell::Cell::new(0),
        })
    }

    #[test]
    fn test_exists_uses_contains_key_not_get() {
        let registry = registry();
        save_bytes(&registry, "big", vec![0; 1024]);
        assert!(exists(&registry, "big"));
        assert!(!exists(&registry, "missing"));
        assert_eq!(registry.borrow().gets.get(), 0);
    }

    #[test]
    fn test_default_contains_key_falls_back_to_get() {
        struct GetOnly(HashMap<String, Vec<u8>>);
        impl StorageRegistry for GetOnly {
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
        let registry = RefCell::new(GetOnly(HashMap::new()));
        save_bytes(&registry, "k", vec![1]);
        assert!(exists(&registry, "k"));
        assert!(!exists(&registry, "other"));
    }

    #[test]
    fn test_save_load_bytes() {
        let registry = registry();

        save_bytes(&registry, "test", vec![1, 2, 3]);
        let loaded = load_bytes(&registry, "test");

        assert_eq!(loaded, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_exists() {
        let registry = registry();

        assert!(!exists(&registry, "test"));
        save_bytes(&registry, "test", vec![1, 2, 3]);
        assert!(exists(&registry, "test"));
    }
}