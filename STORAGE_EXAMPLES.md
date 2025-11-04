# Storage Module Usage Examples

## Overview

The storage module provides **generic, type-safe wrappers** for saving/loading any CandidType to IC stable storage. It eliminates repetitive code by using Rust generics.

## Core Concept

Instead of writing separate functions for each type combination:
```rust
// ❌ OLD WAY - repetitive
save_u8_hashmap(registry, key, map);
save_i8_hashmap(registry, key, map);
save_string_hashmap(registry, key, map);
```

Use generic functions that work with any type:
```rust
// ✅ NEW WAY - one function for all
save_hashmap(registry, "chains", &my_u8_map)?;
save_hashmap(registry, "statuses", &my_i8_map)?;
save_hashmap(registry, "names", &my_string_map)?;
```

## Setup in Your Canister

### 1. Define Your REGISTRIES

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

    pub static REGISTRIES: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        )
    );
}
```

### 2. Implement StorageRegistry Trait

```rust
use ic_dev_kit_rs::storage::StorageRegistry;

impl StorageRegistry for StableBTreeMap<String, Vec<u8>, Memory> {
    fn insert(&mut self, key: String, value: Vec<u8>) {
        self.insert(key, value);
    }
    
    fn get(&self, key: &String) -> Option<Vec<u8>> {
        self.get(key)
    }
    
    fn remove(&mut self, key: &String) -> Option<Vec<u8>> {
        self.remove(key)
    }
}
```

## Usage Patterns

### Example 1: Configuration HashMaps (like your chain IDs)

```rust
use ic_dev_kit_rs::storage;
use std::collections::HashMap;

// Define your config
fn default_chain_ids() -> HashMap<u8, String> {
    let mut map = HashMap::new();
    map.insert(0, "ICP".to_string());
    map.insert(1, "Ethereum".to_string());
    map.insert(2, "Bitcoin".to_string());
    map.insert(3, "Solana".to_string());
    map
}

thread_local! {
    static CHAIN_IDS: RefCell<HashMap<u8, String>> = RefCell::new(default_chain_ids());
}

// Save to stable storage
pub fn save_chain_ids() {
    CHAIN_IDS.with(|map| {
        storage::with_registry_mut(&REGISTRIES, |reg| {
            storage::save_hashmap(reg, "chain_ids", &map.borrow())
        })
    }).unwrap();
}

// Load from stable storage
pub fn load_chain_ids() {
    CHAIN_IDS.with(|map| {
        let loaded: HashMap<u8, String> = storage::with_registry_ref(&REGISTRIES, |reg| {
            storage::load_hashmap(reg, "chain_ids")
        });
        *map.borrow_mut() = loaded;
    });
}
```

### Example 2: Status Maps (like your session/user statuses)

```rust
// Session statuses with i8 keys
fn default_session_statuses() -> HashMap<i8, String> {
    let mut map = HashMap::new();
    map.insert(-1, "rejected".to_string());
    map.insert(0, "pending".to_string());
    map.insert(1, "approved".to_string());
    map
}

thread_local! {
    static SESSION_STATUSES: RefCell<HashMap<i8, String>> = 
        RefCell::new(default_session_statuses());
}

pub fn save_session_statuses() {
    SESSION_STATUSES.with(|map| {
        storage::with_registry_mut(&REGISTRIES, |reg| {
            storage::save_hashmap(reg, "session_statuses", &map.borrow())
        })
    }).unwrap();
}

pub fn load_session_statuses() {
    SESSION_STATUSES.with(|map| {
        let loaded: HashMap<i8, String> = storage::with_registry_ref(&REGISTRIES, |reg| {
            storage::load_hashmap(reg, "session_statuses")
        });
        *map.borrow_mut() = loaded;
    });
}
```

### Example 3: Save All Configs at Once (like your pattern)

```rust
pub fn save_all_config() {
    storage::with_registry_mut(&REGISTRIES, |reg| {
        // Save chain IDs
        CHAIN_IDS.with(|map| {
            let _ = storage::save_hashmap(reg, "chain_ids", &map.borrow());
        });
        
        // Save session statuses
        SESSION_STATUSES.with(|map| {
            let _ = storage::save_hashmap(reg, "session_statuses", &map.borrow());
        });
        
        // Save user statuses
        USER_STATUSES.with(|map| {
            let _ = storage::save_hashmap(reg, "user_statuses", &map.borrow());
        });
    });
}

pub fn load_all_config() {
    // Load chain IDs
    CHAIN_IDS.with(|map| {
        let loaded = storage::with_registry_ref(&REGISTRIES, |reg| {
            storage::load_hashmap(reg, "chain_ids")
        });
        *map.borrow_mut() = loaded;
    });
    
    // Load session statuses
    SESSION_STATUSES.with(|map| {
        let loaded = storage::with_registry_ref(&REGISTRIES, |reg| {
            storage::load_hashmap(reg, "session_statuses")
        });
        *map.borrow_mut() = loaded;
    });
    
    // Load user statuses
    USER_STATUSES.with(|map| {
        let loaded = storage::with_registry_ref(&REGISTRIES, |reg| {
            storage::load_hashmap(reg, "user_statuses")
        });
        *map.borrow_mut() = loaded;
    });
}
```

### Example 4: HashSets (like Principal lists)

```rust
use candid::Principal;
use std::collections::HashSet;

thread_local! {
    static ALLOWED_USERS: RefCell<HashSet<Principal>> = RefCell::new(HashSet::new());
}

pub fn save_allowed_users() {
    ALLOWED_USERS.with(|set| {
        storage::with_registry_mut(&REGISTRIES, |reg| {
            storage::save_hashset(reg, "allowed_users", &set.borrow())
        })
    }).unwrap();
}

pub fn load_allowed_users() {
    ALLOWED_USERS.with(|set| {
        let loaded: HashSet<Principal> = storage::with_registry_ref(&REGISTRIES, |reg| {
            storage::load_hashset(reg, "allowed_users")
        });
        *set.borrow_mut() = loaded;
    });
}

// Or use the convenience function for principals specifically
pub fn save_principals_convenience() {
    ALLOWED_USERS.with(|set| {
        storage::with_registry_mut(&REGISTRIES, |reg| {
            storage::save_principals(reg, "allowed_users", &set.borrow())
        })
    }).unwrap();
}
```

### Example 5: Any Custom Type

```rust
use candid::{CandidType, Deserialize};

#[derive(CandidType, Deserialize, Clone)]
struct MyConfig {
    name: String,
    version: u32,
    features: Vec<String>,
}

pub fn save_my_config(config: &MyConfig) {
    storage::with_registry_mut(&REGISTRIES, |reg| {
        storage::save_data(reg, "my_config", config)
    }).unwrap();
}

pub fn load_my_config() -> Option<MyConfig> {
    storage::with_registry_ref(&REGISTRIES, |reg| {
        storage::load_data(reg, "my_config")
    })
}
```

## In Upgrade Hooks

```rust
#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    // Save all config
    save_all_config();
    
    // Save other data
    save_allowed_users();
    save_my_config(&current_config);
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    // Load all config
    load_all_config();
    
    // Load other data
    load_allowed_users();
    if let Some(config) = load_my_config() {
        // Use config
    }
}
```

## Available Generic Functions

### Core Functions
- `save_data<T>(registry, key, data)` - Save any CandidType
- `load_data<T>(registry, key)` - Load any CandidType
- `save_bytes(registry, key, bytes)` - Save raw bytes
- `load_bytes(registry, key)` - Load raw bytes

### Collection Functions (Generic)
- `save_hashmap<K, V>(registry, key, map)` - Save any HashMap
- `load_hashmap<K, V>(registry, key)` - Load any HashMap
- `save_hashset<T>(registry, key, set)` - Save any HashSet
- `load_hashset<T>(registry, key)` - Load any HashSet

### Convenience Wrappers
- `save_principals(registry, key, set)` - Save HashSet<Principal>
- `load_principals(registry, key)` - Load HashSet<Principal>
- `save_string_hashmap(registry, key, map)` - Save HashMap<String, String>
- `load_string_hashmap(registry, key)` - Load HashMap<String, String>

## Benefits

✅ **Type-safe**: Compiler ensures types match  
✅ **Generic**: Works with any CandidType  
✅ **No repetition**: One function for all HashMap types  
✅ **Testable**: Easy to mock StorageRegistry  
✅ **Flexible**: Works with any storage backend implementing StorageRegistry  

## Migration from Old Code

If you have existing code like:
```rust
// Old
pub fn save_u8_hashmap_to_stable(key: &str, hashmap: &HashMap<u8, String>) {
    let hashmap_vec: Vec<(u8, String)> = hashmap.iter().map(|(k, v)| (*k, v.clone())).collect();
    save_data_to_stable(key, &hashmap_vec);
}
```

Replace with:
```rust
// New
storage::with_registry_mut(&REGISTRIES, |reg| {
    storage::save_hashmap(reg, key, hashmap)
})
```

The generic function handles the Vec conversion automatically!