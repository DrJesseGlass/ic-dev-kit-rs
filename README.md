# ic-dev-kit-rs

Rust toolkit for IC development - standardizes HTTP generation, large object storage, Canister Geek integration, and common canister patterns.

## Features

- **Authentication**: Principal-based authorization with guard functions
- **HTTP Handling**: Request/response types, routing, JSON utilities
- **Telemetry**: Canistergeek integration for monitoring and logging
- **Storage**: Type-safe wrappers for IC stable storage
- **Large Objects**: Chunked upload system for large files
- **Inter-canister Calls**: DRY wrapper with automatic logging
- **Large Objects**: Chunked upload system for large files
- **Inter-canister Calls**: Safe wrappers with timeout, retries, and logging

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ic-dev-kit-rs = "0.1.0"
```

## Quick Start

### 1. Authentication

```rust
use ic_dev_kit_rs::auth;

#[ic_cdk::init]
fn init() {
    // Initialize auth with deployer as authorized principal
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
use ic_dev_kit_rs::http::{self, HttpRequest, HttpResponse};

#[ic_cdk::query]
fn http_request(req: HttpRequest) -> HttpResponse {
    let path = http::extract_path(&req.url);
    
    match (req.method.as_str(), path) {
        ("GET", "/api/status") => {
            http::success_response(&json!({"status": "ok"})).unwrap()
        }
        ("POST", "/api/data") => {
            let data: MyData = http::parse_json(&req.body).unwrap();
            // Process data...
            http::success_response(&data).unwrap()
        }
        _ => http::HttpError::NotFound.to_response(),
    }
}
```

### 3. Telemetry

```rust
use ic_dev_kit_rs::telemetry;

#[ic_cdk::init]
fn init() {
    telemetry::init();
}

#[ic_cdk::update]
fn process_data() {
    telemetry::track_metrics();
    telemetry::log_info("Processing started");
    
    // Your logic here...
    
    telemetry::log_info("Processing completed");
}

#[ic_cdk::query(guard = "telemetry::is_monitoring_authorized")]
fn get_metrics() -> CanisterMetrics {
    telemetry::collect_metrics()
}
```

### 4. Stable Storage Utilities

```rust
use ic_dev_kit_rs::storage;
use ic_stable_structures::{StableBTreeMap, memory_manager::*, DefaultMemoryImpl};
use std::cell::RefCell;

type Memory = VirtualMemory<DefaultMemoryImpl>;

// Define REGISTRIES in your canister
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static REGISTRIES: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        )
    );
}

// Implement StorageRegistry for your StableBTreeMap
impl storage::StorageRegistry for StableBTreeMap<String, Vec<u8>, Memory> {
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

// Use storage utilities
#[ic_cdk::update]
fn save_config(config: MyConfig) -> Result<String, String> {
    storage::with_registry_mut(&REGISTRIES, |reg| {
        storage::save_data(reg, "config", &config)
    })
    .map(|_| "Config saved".to_string())
}

#[ic_cdk::query]
fn get_config() -> Option<MyConfig> {
    storage::with_registry_ref(&REGISTRIES, |reg| {
        storage::load_data(reg, "config")
    })
}
```

### 5. Large Object Uploads

```rust
use ic_dev_kit_rs::large_objects;

// Sequential upload (simple)
#[ic_cdk::update]
fn upload_chunk(data: Vec<u8>) {
    large_objects::append_chunk(data);
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

#[ic_cdk::update]
fn finalize_parallel_upload() -> Result<Vec<u8>, String> {
    large_objects::consolidate_parallel_chunks()?;
    Ok(large_objects::get_buffer_data())
}
```

### 6. Inter-canister Calls

```rust
use ic_dev_kit_rs::intercanister;
use candid::{CandidType, Deserialize, Principal};

#[derive(CandidType)]
struct MyRequest {
    data: String,
}

#[derive(CandidType, Deserialize)]
struct MyResponse {
    result: u64,
}

#[ic_cdk::update]
async fn call_other_canister() -> Result<MyResponse, String> {
    let canister_id = Principal::from_text("...").unwrap();
    
    let request = MyRequest {
        data: "hello".to_string(),
    };
    
    // Automatic logging and error handling
    intercanister::call(canister_id, "my_method", request).await
}

// Call with cycles
#[ic_cdk::update]
async fn call_with_cycles() -> Result<String, String> {
    intercanister::call_with_payment(
        canister_id,
        "paid_method",
        my_request,
        1_000_000, // cycles
    ).await
}
```

### 5. Large File Uploads

```rust
use ic_dev_kit_rs::large_objects;

// Sequential upload (simple)
#[ic_cdk::update]
fn upload_chunk(data: Vec<u8>) {
    large_objects::append_chunk(data);
}

#[ic_cdk::update]
fn finalize_upload() -> Vec<u8> {
    large_objects::get_buffer_data()
}

// Parallel upload (for better performance)
#[ic_cdk::update]
fn upload_parallel_chunk(chunk_id: u32, data: Vec<u8>) {
    large_objects::append_parallel_chunk(chunk_id, data);
}

#[ic_cdk::update]
fn finalize_parallel_upload() -> Result<usize, String> {
    // Consolidates chunks in order and returns total size
    large_objects::consolidate_parallel_chunks()
}

#[ic_cdk::query]
fn upload_status() -> String {
    let status = large_objects::storage_status();
    format!("{}", status)
}
```

### 6. Inter-canister Calls

```rust
use ic_dev_kit_rs::intercanister;
use candid::Principal;

#[ic_cdk::update]
async fn call_other_canister(canister_id: Principal) -> Result<String, String> {
    // Simple call with timeout and automatic logging
    let result: String = intercanister::call_with_timeout(
        canister_id,
        "get_data",
        (),
    )
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(result)
}

#[ic_cdk::update]
async fn call_with_retries_example(canister_id: Principal) -> Result<u64, String> {
    // Automatically retries up to 3 times on transient errors
    let count: u64 = intercanister::call_with_retries(
        canister_id,
        "get_count",
        (),
        3, // max retries
    )
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(count)
}

// Using the macro for less boilerplate
#[ic_cdk::update]
async fn macro_example(canister_id: Principal) -> Result<String, String> {
    let result: String = ic_call!(canister_id, "method_name", ())
        .await
        .map_err(|e| e.to_string())?;
    Ok(result)
}
```

## Upgrade Persistence

All modules support canister upgrades:

```rust
use ic_dev_kit_rs::{auth, telemetry, storage};
use ic_cdk_macros::{pre_upgrade, post_upgrade};

#[pre_upgrade]
fn pre_upgrade() {
    let auth_data = auth::save_to_bytes();
    let telemetry_monitor = telemetry::save_monitor_to_bytes();
    let telemetry_logger = telemetry::save_logger_to_bytes();
    let storage_data = storage::save_to_bytes();
    
    // Save to stable memory...
}

#[post_upgrade]
fn post_upgrade() {
    // Load from stable memory...
    
    auth::init_from_saved(Some(auth_data));
    telemetry::init_from_saved(
        Some(telemetry_monitor),
        Some(telemetry_logger),
        Some(monitoring_principals)
    );
    storage::init();
    storage::load_from_bytes(&storage_data).unwrap();
}
```

## Module Overview

### Authentication (`auth`)

- `init()` - Initialize auth system
- `init_with_caller()` - Initialize with deployer as authorized
- `is_authorized()` - Guard function for IC CDK
- `add_principal()` - Add authorized principal
- `remove_principal()` - Remove authorized principal
- `list_principals()` - List all authorized principals

### HTTP (`http`)

Types:
- `HttpRequest` / `HttpResponse` - IC-compatible HTTP types
- `HttpError` - Rich error types with status codes
- `HttpMethod` - HTTP method enum

Utilities:
- `parse_json()` - Parse request body as JSON
- `success_response()` - Create JSON success response
- `error_response()` - Create error response
- `extract_path()` - Extract path from URL
- `extract_params()` - Extract path parameters
- `get_header()` - Get header value (case-insensitive)

### Telemetry (`telemetry`)

Monitoring:
- `track_metrics()` - Track canister metrics
- `collect_metrics()` - Get current metrics
- `get_information()` - Canistergeek information API

Logging:
- `log_info()` - Log info message
- `log_warning()` - Log warning
- `log_error()` - Log error
- `log_debug()` - Log debug message
- `get_log_messages()` - Retrieve logs

Authorization:
- `is_monitoring_authorized()` - Guard function
- `add_monitoring_principal()` - Add monitoring access
- `list_monitoring_principals()` - List authorized monitors

### Storage (`storage`)

- `save_data()` / `load_data()` - Generic type-safe storage
- `save_hashmap()` / `load_hashmap()` - Save any HashMap<K, V>
- `save_hashset()` / `load_hashset()` - Save any HashSet<T>
- `save_principals()` / `load_principals()` - Convenience for Principal sets
- `save_string_hashmap()` / `load_string_hashmap()` - Convenience for String maps
- Works with any storage backend via `StorageRegistry` trait

See [STORAGE_EXAMPLES.md](./STORAGE_EXAMPLES.md) for detailed usage patterns.

### Large Objects (`large_objects`)

- `append_chunk()` - Append to sequential buffer
- `append_parallel_chunk()` - Add chunk with ID for parallel uploads
- `parallel_chunks_complete()` - Check if all chunks received
- `consolidate_parallel_chunks()` - Merge chunks in order
- `get_buffer_data()` - Get final data
- `missing_chunks()` - Check which chunks are missing
- `storage_status()` - Get upload status

### Inter-canister Calls (`intercanister`)

- `call()` - Basic intercanister call with logging
- `call_with_payment()` - Call with cycles attached
- `call_one_way()` - Fire-and-forget notification
- `call_no_args()` - Convenience for methods with no arguments
- Automatic logging before/after calls
- Consistent error formatting
- DRY: Update timeout/retry logic in one place

### Large Objects (`large_objects`)

- `append_chunk()` - Add chunk to sequential buffer
- `append_parallel_chunk()` - Add chunk with ID for parallel uploads
- `consolidate_parallel_chunks()` - Combine parallel chunks in order
- `get_buffer_data()` - Get and clear buffer data
- `parallel_chunks_complete()` - Validate all chunks received
- `storage_status()` - Monitor upload progress

### Inter-canister Calls (`intercanister`)

- `call_with_timeout()` - Standard call with timeout (recommended)
- `call()` - Basic call without timeout
- `call_with_payment()` - Call with custom cycles payment
- `call_with_retries()` - Automatic retry on transient errors
- `notify()` - One-way notification (no response)
- Macros: `ic_call!()`, `ic_call_retry!()`
- Integrated logging with telemetry module

## Examples

See the [examples](./examples) directory for complete canister examples.

## Contributing

Contributions welcome! Please feel free to submit a Pull Request.

## License

MIT OR Apache-2.0