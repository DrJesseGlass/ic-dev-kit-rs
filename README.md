# ic-dev-kit-rs

Rust toolkit for Internet Computer canister development. Standardizes common patterns: authentication, HTTP handling, storage, telemetry, inter-canister calls, and ML model serving.

## Installation

Published on [crates.io](https://crates.io/crates/ic-dev-kit-rs). Add to your `Cargo.toml`:

```toml
[dependencies]
ic-dev-kit-rs = "0.4"

# Enable optional features (most common)
ic-dev-kit-rs = { version = "0.4", features = ["storage", "telemetry"] }

# ML features (add "storage" too if you use model_server)
ic-dev-kit-rs = { version = "0.4", features = ["text-generation", "storage"] }
```

Requires Rust 1.88 or newer (inherited from `ic-cdk` 0.20).

**Note on `telemetry`:** the published `canistergeek_ic_rust` still requires
`ic-cdk` 0.19, so enabling `telemetry` links a private copy of ic-cdk 0.19
next to your 0.20. Your code stays on 0.20; Canistergeek exposes no ic-cdk
types and both versions share one executor. The cost is a few hundred bytes
of wasm. This goes away once upstream publishes an ic-cdk 0.20 release. The
crate is re-exported as `ic_dev_kit_rs::telemetry::canistergeek_ic_rust`, so
you do not need to depend on it directly.

**Note on ML features + wasm:** the `candle`/`text-generation` features pull in
`getrandom`, which has no default backend on `wasm32-unknown-unknown`. Canister
projects using these features must build with
`RUSTFLAGS='--cfg getrandom_backend="custom"'` (e.g. in `.cargo/config.toml`)
and register a custom entropy source (IC canisters typically seed from
`raw_rand`).

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

// Use the macro to export Canistergeek-compatible endpoints.
// Invoke `ic_cdk::export_candid!()` in this same module: the macro binds the
// Canistergeek types to local aliases so you need no direct dependency on
// `canistergeek_ic_rust`.
ic_dev_kit_rs::export_telemetry_endpoints!();
```

### 5. Large Object Uploads

Buffers are keyed by an `owner` principal, so concurrent uploads from
different callers are isolated from each other. Buffered bytes are capped
both per owner (1 GiB by default, `large_objects::set_max_bytes_per_owner`)
and across all owners (2 GiB by default, `large_objects::set_max_total_bytes`),
since anyone can mint new principals. Buffers live on the Wasm heap —
finalize uploads before upgrading the canister.

```rust
use ic_dev_kit_rs::large_objects;

// Sequential upload (simple, chunks must arrive in order)
#[ic_cdk::update]
fn upload_chunk(data: Vec<u8>) -> Result<usize, String> {
    large_objects::append_chunk(ic_cdk::api::msg_caller(), data)
}

#[ic_cdk::update]
fn finalize_upload() -> Vec<u8> {
    large_objects::get_buffer_data(ic_cdk::api::msg_caller())
}

// Parallel upload (faster, chunks can arrive out of order)
#[ic_cdk::update]
fn upload_parallel_chunk(chunk_id: u32, data: Vec<u8>) -> Result<usize, String> {
    large_objects::append_parallel_chunk(ic_cdk::api::msg_caller(), chunk_id, data)
}

#[ic_cdk::query]
fn check_upload_complete(expected_count: u32) -> bool {
    large_objects::parallel_chunks_complete(ic_cdk::api::msg_caller(), expected_count)
}

#[ic_cdk::query]
fn get_missing_chunks(expected_count: u32) -> Vec<u32> {
    large_objects::missing_chunks(ic_cdk::api::msg_caller(), expected_count)
}

#[ic_cdk::update]
fn finalize_parallel_upload() -> Result<Vec<u8>, String> {
    let owner = ic_cdk::api::msg_caller();
    large_objects::consolidate_parallel_chunks(owner)?;
    Ok(large_objects::get_buffer_data(owner))
}

#[ic_cdk::query]
fn upload_status() -> String {
    large_objects::storage_status(ic_cdk::api::msg_caller()).to_string()
}
```

Or generate all of the above (plus optional storage integration) with the
macro — it passes `msg_caller()` as the owner automatically:

```rust
ic_dev_kit_rs::generate_upload_endpoints!(guard = "auth::is_authorized");
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
| `remove_principal(Principal)` | Remove authorized principal (refuses to remove the last one) |
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
| `exists(registry, key)` | Check if key exists (no value copy; uses `StorageRegistry::contains_key`) |
| `size(registry, key)` | Get size in bytes (reads the value) |

### `large_objects`

All functions take an `owner: Principal` as their first argument (use
`ic_cdk::api::msg_caller()` in endpoints); each owner gets isolated buffers.

| Function | Description |
|----------|-------------|
| `append_chunk(owner, Vec<u8>)` | Add to sequential buffer (checks cap) |
| `buffer_size(owner)` | Get sequential buffer size |
| `get_buffer_data(owner)` | Get and clear sequential buffer |
| `clear_buffer(owner)` | Clear sequential buffer |
| `append_parallel_chunk(owner, u32, Vec<u8>)` | Add chunk with ID (checks cap) |
| `parallel_chunk_count(owner)` | Get parallel chunk count |
| `parallel_chunks_complete(owner, u32)` | Check all chunks received |
| `missing_chunks(owner, u32)` | Get missing chunk IDs |
| `consolidate_parallel_chunks(owner)` | Merge parallel to sequential |
| `get_parallel_data(owner)` | Get parallel data without moving |
| `clear_parallel_chunks(owner)` | Clear parallel buffer |
| `storage_status(owner)` | Get detailed status |
| `total_buffered_bytes(owner)` | Combined buffered bytes for owner |
| `total_buffered_bytes_all_owners()` | Combined buffered bytes across every owner |
| `set_max_bytes_per_owner(Option<usize>)` | Set per-owner byte cap (`None` = unlimited) |
| `max_bytes_per_owner()` | Get the current per-owner cap |
| `set_max_total_bytes(Option<usize>)` | Set the cap across all owners (`None` = unlimited) |
| `max_total_bytes()` | Get the current total cap |

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
| `generate_model_endpoints!(...)` | Generate ML inference endpoints (pass `generate_guard: "fn"` or inference is public) |

## Examples

See the [examples](./examples) directory for complete canister examples.

## Releasing

1. Add a `## [x.y.z]` entry to `CHANGELOG.md` and set `version` in `Cargo.toml`.
   Under semver for 0.x, breaking changes bump the minor version.
2. Commit, then `cargo publish --dry-run` to confirm the package builds.
3. Push a tag `vx.y.z` on that commit. The release workflow checks the tag
   matches `Cargo.toml` and runs `cargo publish` using the
   `CARGO_REGISTRY_TOKEN` repository secret.

## License

MIT OR Apache-2.0