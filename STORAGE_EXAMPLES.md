# Storage Module Usage Examples

## Overview

The storage module provides **type-safe wrappers** for saving/loading any `CandidType` to IC stable storage using Candid serialization.

## Setup

### 1. Define Your Storage Registry

```rust
use ic_stable_structures::{
    StableBTreeMap,
    memory_manager::{MemoryManager, MemoryId, VirtualMemory},
    DefaultMemoryImpl
};
use std::cell::RefCell;

type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    pub static REGISTRY: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        )
    );
}
```

### 2. Use Storage Functions

The `StorageRegistry` trait is already implemented for `StableBTreeMap<String, Vec<u8>, Memory>`, so you can use the storage functions directly.

## Core Functions

### `save_candid` / `load_candid`

Save and load any type that implements `CandidType`:

```rust
use ic_dev_kit_rs::storage;
use candid::{CandidType, Deserialize};

#[derive(CandidType, Deserialize, Clone)]
struct MyConfig {
    name: String,
    version: u32,
    features: Vec<String>,
}

// Save
pub fn save_config(config: &MyConfig) -> Result<(), String> {
    REGISTRY.with(|reg| {
        storage::save_candid(reg, "config", config)
    })
}

// Load
pub fn load_config() -> Option<MyConfig> {
    REGISTRY.with(|reg| {
        storage::load_candid(reg, "config")
    })
}
```

### `save_bytes` / `load_bytes`

For raw binary data (model weights, images, etc.):

```rust
use ic_dev_kit_rs::storage;

pub fn save_model_weights(weights: Vec<u8>) {
    REGISTRY.with(|reg| {
        storage::save_bytes(reg, "model_weights", weights);
    });
}

pub fn load_model_weights() -> Option<Vec<u8>> {
    REGISTRY.with(|reg| {
        storage::load_bytes(reg, "model_weights")
    })
}
```

### Utility Functions

```rust
use ic_dev_kit_rs::storage;

// Check if key exists
pub fn has_config() -> bool {
    REGISTRY.with(|reg| storage::exists(reg, "config"))
}

// Get size of stored data
pub fn get_config_size() -> Option<usize> {
    REGISTRY.with(|reg| storage::size(reg, "config"))
}

// Delete entry
pub fn delete_config() -> bool {
    REGISTRY.with(|reg| storage::delete(reg, "config"))
}
```

## Common Patterns

### Saving Collections

Since `save_candid` works with any `CandidType`, you can save collections directly:

```rust
use std::collections::HashMap;
use candid::Principal;

// HashMap
pub fn save_chain_ids(map: &HashMap<u8, String>) -> Result<(), String> {
    REGISTRY.with(|reg| {
        storage::save_candid(reg, "chain_ids", map)
    })
}

pub fn load_chain_ids() -> Option<HashMap<u8, String>> {
    REGISTRY.with(|reg| {
        storage::load_candid(reg, "chain_ids")
    })
}

// Vec of Principals
pub fn save_allowed_users(users: &Vec<Principal>) -> Result<(), String> {
    REGISTRY.with(|reg| {
        storage::save_candid(reg, "allowed_users", users)
    })
}

pub fn load_allowed_users() -> Option<Vec<Principal>> {
    REGISTRY.with(|reg| {
        storage::load_candid(reg, "allowed_users")
    })
}
```

### Multiple Configs at Once

```rust
pub fn save_all_config(
    chain_ids: &HashMap<u8, String>,
    session_statuses: &HashMap<i8, String>,
    allowed_users: &Vec<Principal>,
) {
    REGISTRY.with(|reg| {
        let _ = storage::save_candid(reg, "chain_ids", chain_ids);
        let _ = storage::save_candid(reg, "session_statuses", session_statuses);
        let _ = storage::save_candid(reg, "allowed_users", allowed_users);
    });
}
```

### Thread-Local State with Persistence

```rust
use std::collections::HashMap;

fn default_chain_ids() -> HashMap<u8, String> {
    let mut map = HashMap::new();
    map.insert(0, "ICP".to_string());
    map.insert(1, "Ethereum".to_string());
    map
}

thread_local! {
    static CHAIN_IDS: RefCell<HashMap<u8, String>> = RefCell::new(default_chain_ids());
}

// Save current state to stable storage
pub fn persist_chain_ids() {
    CHAIN_IDS.with(|ids| {
        REGISTRY.with(|reg| {
            let _ = storage::save_candid(reg, "chain_ids", &*ids.borrow());
        });
    });
}

// Load from stable storage into thread-local
pub fn restore_chain_ids() {
    if let Some(loaded) = REGISTRY.with(|reg| {
        storage::load_candid::<HashMap<u8, String>>(reg, "chain_ids")
    }) {
        CHAIN_IDS.with(|ids| {
            *ids.borrow_mut() = loaded;
        });
    }
}
```

## Upgrade Hooks

```rust
#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    // Save thread-local state to stable storage
    persist_chain_ids();
    persist_session_statuses();
    persist_allowed_users();
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    // Restore thread-local state from stable storage
    restore_chain_ids();
    restore_session_statuses();
    restore_allowed_users();
}
```

## Integration with Large Objects

When using `large_objects` with storage, you can save uploaded data directly:

```rust
use ic_dev_kit_rs::{large_objects, storage};

#[ic_cdk::update]
fn finalize_and_save(key: String) -> Result<String, String> {
    // Get data from upload buffer
    let data = large_objects::get_buffer_data();
    if data.is_empty() {
        return Err("No data in buffer".to_string());
    }
    
    let size = data.len();
    
    // Save to stable storage
    REGISTRY.with(|reg| {
        storage::save_bytes(reg, &key, data);
    });
    
    Ok(format!("Saved {} bytes to '{}'", size, key))
}
```

Or use the `generate_upload_endpoints!` macro which handles this automatically:

```rust
ic_dev_kit_rs::generate_upload_endpoints!(
    guard = "auth::is_authorized",
    registry = REGISTRY
);
```

## API Reference

| Function | Description |
|----------|-------------|
| `save_candid<T>(registry, key, &T)` | Save any CandidType with Candid serialization |
| `load_candid<T>(registry, key) -> Option<T>` | Load and deserialize a CandidType |
| `save_bytes(registry, key, Vec<u8>)` | Save raw bytes |
| `load_bytes(registry, key) -> Option<Vec<u8>>` | Load raw bytes |
| `exists(registry, key) -> bool` | Check if key exists |
| `size(registry, key) -> Option<usize>` | Get size of stored value |
| `delete(registry, key) -> bool` | Delete entry, returns true if existed |