// Generic storage utilities for IC stable structures
//
// This module provides type-safe wrappers around IC stable storage.
//
// ## Usage
//
// Users need to define their own REGISTRIES in their canister's lib.rs:
//
// ```rust
// use ic_stable_structures::{
//     StableBTreeMap,
//     memory_manager::{MemoryManager, MemoryId, VirtualMemory},
//     DefaultMemoryImpl
// };
// use std::cell::RefCell;
//
// type Memory = VirtualMemory<DefaultMemoryImpl>;
//
// thread_local! {
//     static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
//         RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
//
//     pub static REGISTRIES: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
//         StableBTreeMap::init(
//             MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
//         )
//     );
// }
// ```
//
// Then use the storage functions by passing your REGISTRIES:
//
// ```rust
// use ic_dev_kit_rs::storage;
//
// // Save data
// storage::with_registry(&REGISTRIES, |reg| {
//     storage::save_data(reg, "my_key", &my_data);
// });
//
// // Load data
// let data: Option<MyType> = storage::with_registry(&REGISTRIES, |reg| {
//     storage::load_data(reg, "my_key")
// });
// ```

use candid::{CandidType, Encode, Decode, Principal};
use std::collections::{HashSet, HashMap};
use std::cell::RefCell;

// ═══════════════════════════════════════════════════════════════
//  Core Storage Functions (Generic)
// ═══════════════════════════════════════════════════════════════

/// Save any CandidType to a registry
///
/// This is a generic utility that can serialize any CandidType to stable storage.
pub fn save_data<T, R>(registry: &mut R, key: &str, data: &T) -> Result<(), String>
where
    T: CandidType,
    R: StorageRegistry,
{
    match Encode!(data) {
        Ok(serialized_bytes) => {
            registry.insert(key.to_string(), serialized_bytes);
            Ok(())
        }
        Err(e) => {
            Err(format!("Failed to serialize data for key {}: {:?}", key, e))
        }
    }
}

/// Load any CandidType from a registry
pub fn load_data<T, R>(registry: &R, key: &str) -> Option<T>
where
    T: for<'de> candid::Deserialize<'de> + CandidType,
    R: StorageRegistry,
{
    if let Some(serialized_bytes) = registry.get(&key.to_string()) {
        match Decode!(&serialized_bytes, T) {
            Ok(data) => Some(data),
            Err(e) => {
                eprintln!("Failed to deserialize data for key {}: {:?}", key, e);
                None
            }
        }
    } else {
        None
    }
}

/// Save raw bytes directly
pub fn save_bytes<R>(registry: &mut R, key: &str, bytes: Vec<u8>) -> Result<(), String>
where
    R: StorageRegistry,
{
    registry.insert(key.to_string(), bytes);
    Ok(())
}

/// Load raw bytes directly
pub fn load_bytes<R>(registry: &R, key: &str) -> Option<Vec<u8>>
where
    R: StorageRegistry,
{
    registry.get(&key.to_string())
}

// ═══════════════════════════════════════════════════════════════
//  Storage Registry Trait
// ═══════════════════════════════════════════════════════════════

/// Trait for storage registries
///
/// Implement this for your StableBTreeMap or other storage backend
pub trait StorageRegistry {
    fn insert(&mut self, key: String, value: Vec<u8>);
    fn get(&self, key: &String) -> Option<Vec<u8>>;
    fn remove(&mut self, key: &String) -> Option<Vec<u8>>;
}

// ═══════════════════════════════════════════════════════════════
//  Generic Collection Functions
// ═══════════════════════════════════════════════════════════════

/// Save any HashMap to stable storage (generic over key and value types)
///
/// Works with any HashMap where K and V implement CandidType + Clone
pub fn save_hashmap<K, V, R>(registry: &mut R, key: &str, hashmap: &HashMap<K, V>) -> Result<(), String>
where
    K: Clone + CandidType,
    V: Clone + CandidType,
    R: StorageRegistry,
{
    let hashmap_vec: Vec<(K, V)> = hashmap.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    save_data(registry, key, &hashmap_vec)
}

/// Load any HashMap from stable storage (generic over key and value types)
///
/// Returns an empty HashMap if the key doesn't exist or deserialization fails
pub fn load_hashmap<K, V, R>(registry: &R, key: &str) -> HashMap<K, V>
where
    K: Eq + std::hash::Hash + for<'de> candid::Deserialize<'de> + CandidType,
    V: for<'de> candid::Deserialize<'de> + CandidType,
    R: StorageRegistry,
{
    load_data::<Vec<(K, V)>, R>(registry, key)
        .map(|vec| vec.into_iter().collect())
        .unwrap_or_else(HashMap::new)
}

/// Save any HashSet to stable storage (generic over element type)
pub fn save_hashset<T, R>(registry: &mut R, key: &str, set: &HashSet<T>) -> Result<(), String>
where
    T: Clone + CandidType,
    R: StorageRegistry,
{
    let vec: Vec<T> = set.iter().cloned().collect();
    save_data(registry, key, &vec)
}

/// Load any HashSet from stable storage (generic over element type)
pub fn load_hashset<T, R>(registry: &R, key: &str) -> HashSet<T>
where
    T: Eq + std::hash::Hash + for<'de> candid::Deserialize<'de> + CandidType,
    R: StorageRegistry,
{
    load_data::<Vec<T>, R>(registry, key)
        .map(|vec| vec.into_iter().collect())
        .unwrap_or_else(HashSet::new)
}

// ═══════════════════════════════════════════════════════════════
//  Convenience Type Aliases (for common patterns)
// ═══════════════════════════════════════════════════════════════

/// Save HashSet<Principal> - convenience wrapper
pub fn save_principals<R>(registry: &mut R, key: &str, principals: &HashSet<Principal>) -> Result<(), String>
where
    R: StorageRegistry,
{
    save_hashset(registry, key, principals)
}

/// Load HashSet<Principal> - convenience wrapper
pub fn load_principals<R>(registry: &R, key: &str) -> HashSet<Principal>
where
    R: StorageRegistry,
{
    load_hashset(registry, key)
}

/// Save HashMap<String, String> - convenience wrapper
pub fn save_string_hashmap<R>(registry: &mut R, key: &str, hashmap: &HashMap<String, String>) -> Result<(), String>
where
    R: StorageRegistry,
{
    save_hashmap(registry, key, hashmap)
}

/// Load HashMap<String, String> - convenience wrapper
pub fn load_string_hashmap<R>(registry: &R, key: &str) -> HashMap<String, String>
where
    R: StorageRegistry,
{
    load_hashmap(registry, key)
}

// ═══════════════════════════════════════════════════════════════
//  Helper for thread_local! REGISTRIES
// ═══════════════════════════════════════════════════════════════

/// Helper to work with thread_local REGISTRIES wrapped in RefCell
///
/// Example:
/// ```rust
/// thread_local! {
///     static REGISTRIES: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = ...;
/// }
///
/// // Use it:
/// storage::with_registry(&REGISTRIES, |reg| {
///     storage::save_data(reg, "key", &data)
/// });
/// ```
pub fn with_registry_ref<T, F, R>(registry: &'static std::thread::LocalKey<RefCell<R>>, f: F) -> T
where
    F: FnOnce(&R) -> T,
    R: 'static,
{
    registry.with(|r| {
        let reg_ref = r.borrow();
        f(&*reg_ref)
    })
}

/// Helper to work with thread_local REGISTRIES wrapped in RefCell (mutable access)
pub fn with_registry_mut<T, F, R>(registry: &'static std::thread::LocalKey<RefCell<R>>, f: F) -> T
where
    F: FnOnce(&mut R) -> T,
    R: 'static,
{
    registry.with(|r| {
        let mut reg_mut = r.borrow_mut();
        f(&mut *reg_mut)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock storage for testing
    struct MockRegistry {
        data: HashMap<String, Vec<u8>>,
    }

    impl MockRegistry {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl StorageRegistry for MockRegistry {
        fn insert(&mut self, key: String, value: Vec<u8>) {
            self.data.insert(key, value);
        }

        fn get(&self, key: &String) -> Option<Vec<u8>> {
            self.data.get(key).cloned()
        }

        fn remove(&mut self, key: &String) -> Option<Vec<u8>> {
            self.data.remove(key)
        }
    }

    #[test]
    fn test_save_and_load_data() {
        let mut registry = MockRegistry::new();

        let data = vec![1u32, 2, 3, 4, 5];
        save_data(&mut registry, "test_key", &data).unwrap();

        let loaded: Option<Vec<u32>> = load_data(&registry, "test_key");
        assert_eq!(loaded, Some(data));
    }

    #[test]
    fn test_save_and_load_principals() {
        let mut registry = MockRegistry::new();

        let mut principals = HashSet::new();
        principals.insert(Principal::anonymous());

        save_principals(&mut registry, "principals", &principals).unwrap();
        let loaded = load_principals(&registry, "principals");

        assert_eq!(loaded, principals);
    }

    #[test]
    fn test_generic_hashmap_string_string() {
        let mut registry = MockRegistry::new();

        let mut map = HashMap::new();
        map.insert("key1".to_string(), "value1".to_string());
        map.insert("key2".to_string(), "value2".to_string());

        save_hashmap(&mut registry, "map", &map).unwrap();
        let loaded: HashMap<String, String> = load_hashmap(&registry, "map");

        assert_eq!(loaded, map);
    }

    #[test]
    fn test_generic_hashmap_u8_string() {
        let mut registry = MockRegistry::new();

        let mut map = HashMap::new();
        map.insert(0u8, "ICP".to_string());
        map.insert(1u8, "Ethereum".to_string());
        map.insert(2u8, "Bitcoin".to_string());

        save_hashmap(&mut registry, "chains", &map).unwrap();
        let loaded: HashMap<u8, String> = load_hashmap(&registry, "chains");

        assert_eq!(loaded, map);
    }

    #[test]
    fn test_generic_hashmap_i8_string() {
        let mut registry = MockRegistry::new();

        let mut map = HashMap::new();
        map.insert(-1i8, "rejected".to_string());
        map.insert(0i8, "pending".to_string());
        map.insert(1i8, "approved".to_string());

        save_hashmap(&mut registry, "statuses", &map).unwrap();
        let loaded: HashMap<i8, String> = load_hashmap(&registry, "statuses");

        assert_eq!(loaded, map);
    }

    #[test]
    fn test_generic_hashset() {
        let mut registry = MockRegistry::new();

        let mut set = HashSet::new();
        set.insert("apple".to_string());
        set.insert("banana".to_string());
        set.insert("cherry".to_string());

        save_hashset(&mut registry, "fruits", &set).unwrap();
        let loaded: HashSet<String> = load_hashset(&registry, "fruits");

        assert_eq!(loaded, set);
    }

    #[test]
    fn test_save_and_load_string_hashmap() {
        let mut registry = MockRegistry::new();

        let mut map = HashMap::new();
        map.insert("key1".to_string(), "value1".to_string());
        map.insert("key2".to_string(), "value2".to_string());

        save_string_hashmap(&mut registry, "map", &map).unwrap();
        let loaded = load_string_hashmap(&registry, "map");

        assert_eq!(loaded, map);
    }

    #[test]
    fn test_load_nonexistent() {
        let registry = MockRegistry::new();

        let loaded: Option<Vec<u32>> = load_data(&registry, "nonexistent");
        assert_eq!(loaded, None);

        let loaded_principals = load_principals(&registry, "nonexistent");
        assert_eq!(loaded_principals, HashSet::new());

        let loaded_map: HashMap<String, String> = load_hashmap(&registry, "nonexistent");
        assert_eq!(loaded_map, HashMap::new());

        let loaded_set: HashSet<String> = load_hashset(&registry, "nonexistent");
        assert_eq!(loaded_set, HashSet::new());
    }
}