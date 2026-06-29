# ic-dev-kit-rs

Rust toolkit for Internet Computer canister development. Standardizes common patterns: authentication, HTTP handling, storage, telemetry, inter-canister calls, and ML model serving.

## Installation

Released via git tags (not yet on crates.io). Add to your `Cargo.toml`:

```toml
[dependencies]
ic-dev-kit-rs = { git = "https://github.com/DrJesseGlass/ic-dev-kit-rs", tag = "v0.1.0" }

# Enable optional features (most common)
ic-dev-kit-rs = { git = "https://github.com/DrJesseGlass/ic-dev-kit-rs", tag = "v0.1.0", features = ["storage", "telemetry"] }

# ML features (includes storage automatically)
ic-dev-kit-rs = { git = "https://github.com/DrJesseGlass/ic-dev-kit-rs", tag = "v0.1.0", features = ["text-generation"] }
```

## Features

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `storage` | Stable storage utilities | `ic-stable-structures` |
| `telemetry` | Canistergeek monitoring/logging | `canistergeek_ic_rust` |
| `candle` | ML model infrastructure | `candle-core`, `candle-nn` |
| `text-generation` | LLM text generation | `candle`, `tokenizers` |

## Quick Start

### 1. Authentication

```rust
use ic_dev_kit_rs::auth;

#[ic_cdk::init]
fn init() {
    // Initialize auth with deployer as first authorized principal
    auth::init_with_caller();
}

#[ic_cdk::update(guard = "auth::is_authorized")]
fn protected_method() {
    // Only authorized principals can call this
}

#[ic_cdk::update(guard = "auth::is_authorized")]
fn add_admin(principal: Principal) {
    auth::add_principal(principal).unwrap();
}
```

### 2. HTTP Handling

```rust
use ic_dev_kit_rs::http::{self, HttpRequest, HttpResponse, HttpError};

#[ic_cdk::query]
fn http_request(req: HttpRequest) -> HttpResponse {
    let path = http::extract_path(&req.url);
    
    match (req.method.as_str(), path) {
        ("GET", "/api/status") => {
            http::success_response(&serde_json::json!({"status": "ok"})).unwrap()
        }
        ("POST", "/api/data") => {
            match http::parse_json::<MyData>(&req.body) {
                Ok(data) => http::success_response(&data).unwrap(),
                Err(e) => e.to_response(),
            }
        }
        _ => HttpError::NotFound.to_response(),
    }
}
```

### 3. Storage (requires `storage` feature)

```rust
use ic_dev_kit_rs::storage::{self, StorageRegistry};
use ic_stable_structures::{StableBTreeMap, memory_manager::*, DefaultMemoryImpl};
use std::cell::RefCell;

type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static REGISTRY: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        )
    );
}

// Save any CandidType
#[ic_cdk::update]
fn save_config(config: MyConfig) -> Result<(), String> {
    REGISTRY.with(|reg| {
        storage::save_candid(reg, "config", &config)
    })
}

// Load it back
#[ic_cdk::query]
fn get_config() -> Option<MyConfig> {
    REGISTRY.with(|reg| {
        storage::load_candid(reg, "config")
    })
}

// Raw bytes for binary data
#[ic_cdk::update]
fn save_binary(key: String, data: Vec<u8>) {
    REGISTRY.with(|reg| {
        storage::save_bytes(reg, &key, data);
    });
}
```

### 4. Telemetry (requires `telemetry` feature)

```rust
use ic_dev_kit_rs::telemetry;

#[ic_cdk::init]
fn init() {
    telemetry::init();
}

#[ic_cdk::update]
fn process_data() {
    telemetry::collect_metrics();
    telemetry::log_info("Processing started");
    
    // Your logic here...
    
    telemetry::log_info("Processing completed");
}

// Use the macro to export Canistergeek-compatible endpoints
ic_dev_kit_rs::export_telemetry_endpoints!();
```

### 5. Large Object Uploads

```rust
use ic_dev_kit_rs::large_objects;

// Sequential upload (simple, chunks must arrive in order)
#[ic_cdk::update]
fn upload_chunk(data: Vec<u8>) -> usize {
    large_objects::append_chunk(data);
    large_objects::buffer_size()
}

#[ic_cdk::update]
fn finalize_upload() -> Vec<u8> {
    large_objects::get_buffer_data()
}

// Parallel upload (faster, chunks can arrive out of order)
#[ic_cdk::update]
fn upload_parallel_chunk(chunk_id: u32, data: Vec<u8>) {
    large_objects::append_parallel_chunk(chunk_id, data);
}

#[ic_cdk::query]
fn check_upload_complete(expected_count: u32) -> bool {
    large_objects::parallel_chunks_complete(expected_count)
}

#[ic_cdk::query]
fn get_missing_chunks(expected_count: u32) -> Vec<u32> {
    large_objects::missing_chunks(expected_count)
}

#[ic_cdk::update]
fn finalize_parallel_upload() -> Result<Vec<u8>, String> {
    large_objects::consolidate_parallel_chunks()?;
    Ok(large_objects::get_buffer_data())
}

#[ic_cdk::query]
fn upload_status() -> String {
    large_objects::storage_status().to_string()
}
```

### 6. Inter-canister Calls

```rust
use ic_dev_kit_rs::intercanister;
use candid::Principal;

#[ic_cdk::update]
async fn call_other_canister(canister_id: Principal) -> Result<String, String> {
    // Simple call with automatic logging
    intercanister::call(canister_id, "get_data", ()).await
}

#[ic_cdk::update]
async fn call_with_args(canister_id: Principal, arg: String) -> Result<u64, String> {
    // Call with arguments (use tuple for multiple args)
    intercanister::call(canister_id, "process", (arg,)).await
}

#[ic_cdk::update]
async fn call_with_cycles(canister_id: Principal) -> Result<String, String> {
    // Call with cycles attached
    intercanister::call_with_payment(
        canister_id,
        "paid_method",
        (),
        1_000_000, // cycles
    ).await
}

#[ic_cdk::update]
fn fire_and_forget(canister_id: Principal) -> Result<(), String> {
    // One-way notification (no response)
    intercanister::call_one_way(canister_id, "log_event", ("user_action",))
}
```

## Upgrade Persistence

All modules support canister upgrades:

```rust
use ic_dev_kit_rs::auth;

#[cfg(feature = "telemetry")]
use ic_dev_kit_rs::telemetry;

// Store bytes in stable memory (use ic-stable-structures or similar)
thread_local! {
    static AUTH_BACKUP: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    #[cfg(feature = "telemetry")]
    static TELEMETRY_BACKUP: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    AUTH_BACKUP.with(|b| *b.borrow_mut() = auth::save_to_bytes());
    
    #[cfg(feature = "telemetry")]
    TELEMETRY_BACKUP.with(|b| *b.borrow_mut() = telemetry::save_to_bytes());
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    let auth_data = AUTH_BACKUP.with(|b| b.borrow().clone());
    auth::init_from_saved(if auth_data.is_empty() { None } else { Some(auth_data) });
    
    #[cfg(feature = "telemetry")]
    {
        let telemetry_data = TELEMETRY_BACKUP.with(|b| b.borrow().clone());
        telemetry::init_from_bytes(if telemetry_data.is_empty() { None } else { Some(telemetry_data) });
    }
}
```

## Module Reference

### `auth`

| Function | Description |
|----------|-------------|
| `init()` | Initialize with empty auth |
| `init_with_caller()` | Initialize with deployer authorized |
| `init_with_principals(Vec<Principal>)` | Initialize with specific principals |
| `init_from_saved(Option<Vec<u8>>)` | Restore from saved bytes |
| `is_authorized() -> Result<(), String>` | Guard function for IC CDK |
| `add_principal(Principal)` | Add authorized principal |
| `remove_principal(Principal)` | Remove authorized principal |
| `list_principals()` | List all authorized principals |
| `save_to_bytes() -> Vec<u8>` | Serialize for upgrade |

### `http`

**Types:** `HttpRequest`, `HttpResponse`, `HttpError`, `HttpMethod`, `Router`

| Function | Description |
|----------|-------------|
| `parse_json<T>(&[u8])` | Parse request body as JSON |
| `success_response<T>(&T)` | Create 200 JSON response |
| `error_response(u16, &str)` | Create error response |
| `json_response(u16, String)` | Create JSON response with status |
| `extract_path(&str)` | Extract path from URL |
| `extract_query_params(&str)` | Extract query parameters |
| `extract_params(&str, &str)` | Extract path parameters from pattern |
| `matches_pattern(&str, &str)` | Check if path matches pattern |
| `get_header(&[(String,String)], &str)` | Get header (case-insensitive) |
| `extract_bearer_token(&[(String,String)])` | Extract Bearer token |

### `storage` (feature: `storage`)

| Function | Description |
|----------|-------------|
| `save_candid<T>(registry, key, &T)` | Save any CandidType |
| `load_candid<T>(registry, key)` | Load any CandidType |
| `save_bytes(registry, key, Vec<u8>)` | Save raw bytes |
| `load_bytes(registry, key)` | Load raw bytes |
| `delete(registry, key)` | Delete entry |
| `exists(registry, key)` | Check if key exists |
| `size(registry, key)` | Get size in bytes |

### `large_objects`

| Function | Description |
|----------|-------------|
| `append_chunk(Vec<u8>)` | Add to sequential buffer |
| `buffer_size()` | Get sequential buffer size |
| `get_buffer_data()` | Get and clear sequential buffer |
| `clear_buffer()` | Clear sequential buffer |
| `append_parallel_chunk(u32, Vec<u8>)` | Add chunk with ID |
| `parallel_chunk_count()` | Get parallel chunk count |
| `parallel_chunks_complete(u32)` | Check all chunks received |
| `missing_chunks(u32)` | Get missing chunk IDs |
| `consolidate_parallel_chunks()` | Merge parallel to sequential |
| `get_parallel_data()` | Get parallel data without moving |
| `clear_parallel_chunks()` | Clear parallel buffer |
| `storage_status()` | Get detailed status |

### `intercanister`

| Function | Description |
|----------|-------------|
| `call<T,R>(Principal, &str, T)` | Async call with logging |
| `call_with_payment<T,R>(Principal, &str, T, u128)` | Call with cycles |
| `call_one_way<T>(Principal, &str, T)` | Fire-and-forget notification |
| `call_no_args<R>(Principal, &str)` | Call with no arguments |

### `telemetry` (feature: `telemetry`)

| Function | Description |
|----------|-------------|
| `init()` | Initialize telemetry |
| `collect_metrics()` | Collect canister metrics |
| `log_info(msg)` | Log info message |
| `log_warning(msg)` | Log warning message |
| `log_error(msg)` | Log error message |
| `log_debug(msg)` | Log debug message |
| `is_monitoring_authorized()` | Guard for monitoring endpoints |
| `add_monitoring_principal(Principal)` | Add monitoring access |
| `save_to_bytes()` | Serialize for upgrade |
| `init_from_bytes(Option<Vec<u8>>)` | Restore from saved bytes |

## Macros

| Macro | Description |
|-------|-------------|
| `export_auth_endpoints!()` | Generate auth management endpoints |
| `export_telemetry_endpoints!()` | Generate Canistergeek endpoints |
| `generate_upload_endpoints!(...)` | Generate upload endpoints |
| `generate_model_endpoints!(...)` | Generate ML inference endpoints |

## Examples

See the [examples](./examples) directory for complete canister examples.

## License

MIT OR Apache-2.0