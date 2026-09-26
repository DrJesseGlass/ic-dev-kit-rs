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

Requires Rust 1.94 or newer (`candle-core` 0.11 needs it on aarch64).

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
| `candle` | ML model traits and GGUF helpers | `candle-core`, `candle-nn` |
| `text-generation` | LLM generation loop and tokenizer helpers | `candle`, `candle-transformers`, `tokenizers` |

The `model_server` module (ready-made LLM server plus `generate_model_endpoints!`)
is compiled only when **both** `text-generation` and `storage` are enabled.
Modules without a feature (`auth`, `http`, `large_objects`, `intercanister`) are
always available.

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

`http::Router` does the same with pattern routes (`/api/users/:id`, `/api/*`).
An exact path match wins; otherwise patterns are tried in the order they were
registered, so register specific patterns before broad ones.

#### Streaming large responses

The IC caps a single response at ~2 MiB. For larger bodies, return the first
chunk with a `StreamingStrategy` and the gateway calls your callback query for
the rest until it returns a response with no token:

```rust
use ic_dev_kit_rs::http::{
    HttpRequest, HttpResponse, StreamingCallback, StreamingCallbackHttpResponse,
    StreamingCallbackToken, StreamingStrategy,
};

// Your chunking: (bytes for `index`, whether more follow)
fn chunk_for(key: &str, index: u64) -> (Vec<u8>, bool) { /* ... */ }

#[ic_cdk::query]
fn http_request(req: HttpRequest) -> HttpResponse {
    let (first_chunk, has_more) = chunk_for(&req.url, 0);
    let mut response = HttpResponse::new(200, vec![], first_chunk);
    if has_more {
        response = response.with_streaming_strategy(StreamingStrategy::Callback {
            callback: StreamingCallback::new(
                ic_cdk::api::canister_self(),
                "http_request_streaming_callback".to_string(),
            ),
            token: StreamingCallbackToken {
                key: req.url.clone(),
                content_encoding: "identity".to_string(),
                index: 1u64.into(),
                sha256: None,
            },
        });
    }
    response
}

#[ic_cdk::query]
fn http_request_streaming_callback(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    let index = u64::try_from(&token.index.0).unwrap_or(u64::MAX);
    let (body, has_more) = chunk_for(&token.key, index);
    StreamingCallbackHttpResponse {
        body,
        token: has_more.then(|| StreamingCallbackToken { index: (index + 1).into(), ..token }),
    }
}
```

`StreamingCallbackToken` follows the certified asset canister field layout, so
existing gateway tooling understands it. `streaming_strategy` is present on the
Candid wire (what the gateway reads) but omitted from JSON serialization.

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

### 7. ML Model Serving (requires `text-generation` + `storage`)

`candle` provides the model traits and GGUF loading helpers, `text_generation`
the autoregressive generation loop with IC instruction-budget handling, and
`model_server` a ready-made server that loads weights and tokenizer from a
storage registry. Implement `CandleModel` and `AutoregressiveModel` for your
model (see the rustdoc for both traits), then:

```rust
use ic_dev_kit_rs::{auth, model_server::ModelServer};

thread_local! {
    static MODEL_SERVER: ModelServer<MyLlm> = ModelServer::new();
    // REGISTRY: a StableBTreeMap<String, Vec<u8>, _> as in section 3
}

// setup_model / reset_generation (admin), generate (guarded by generate_guard),
// is_model_loaded / get_model_info (public)
ic_dev_kit_rs::generate_model_endpoints!(
    server: MODEL_SERVER,
    registry: REGISTRY,
    weights_key: "model_weights",
    tokenizer_key: "tokenizer",
    get_tokenizer: |model| Box::new(model.tokenizer_handle()),
    generate_guard: "auth::is_authorized"   // omit and `generate` is public
);

// Chunked upload of the weights into REGISTRY (see section 5)
ic_dev_kit_rs::generate_upload_endpoints!(guard = "auth::is_authorized", registry = REGISTRY);
```

Generation stops on EOS, on `max_tokens`, or when the call approaches the IC
instruction budget (`text_generation::INSTRUCTION_LIMIT`); the response says
which. Remember the `getrandom` note in the installation section for wasm
builds. Invoke `ic_cdk::export_candid!()` in the same module as these macros.

## Upgrade Persistence

`auth` and `telemetry` keep their state on the Wasm heap, which an upgrade
wipes. Serialize both in `pre_upgrade` into **stable** memory and restore in
`post_upgrade`. With the `storage` and `telemetry` features and the `REGISTRY`
from section 3:

```rust
use ic_dev_kit_rs::{auth, storage, telemetry};

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    REGISTRY.with(|reg| {
        storage::save_bytes(reg, "__auth__", auth::save_to_bytes());
        storage::save_bytes(reg, "__telemetry__", telemetry::save_to_bytes());
    });
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    REGISTRY.with(|reg| {
        auth::init_from_saved(storage::load_bytes(reg, "__auth__"));
        telemetry::init_from_bytes(storage::load_bytes(reg, "__telemetry__"));
    });
}
```

Without `storage`, write the bytes with `ic_cdk::stable` (or any stable-memory
structure) instead. A `thread_local!` buffer is **not** enough: it lives on the
heap and is gone after the upgrade.

If `init_from_saved` receives `None` or undecodable bytes it authorizes the
caller of the upgrade, so a lost allowlist never locks you out; the fallback is
logged. `telemetry::init_from_bytes` falls back to a fresh, empty state. The
[example canister](./examples/simple_counter) does exactly this.

## Module Reference

### `auth`

| Function | Description |
|----------|-------------|
| `init()` | Initialize with empty auth |
| `init_with_caller()` | Initialize with deployer authorized |
| `init_with_principals(Vec<Principal>)` | Initialize with specific principals |
| `init_from_saved(Option<Vec<u8>>)` | Restore from saved bytes (falls back to authorizing the caller) |
| `is_authorized() -> Result<(), String>` | Guard function for IC CDK |
| `add_principal(Principal)` | Add authorized principal |
| `remove_principal(Principal)` | Remove authorized principal (refuses to remove the last one) |
| `is_principal_authorized(Principal)` | Check a specific principal |
| `list_principals()` | List all authorized principals |
| `save_to_bytes() -> Vec<u8>` | Serialize for upgrade |
| `load_from_bytes(&[u8])` | Replace the set from saved bytes (errors instead of falling back) |
| `validate_principal_text(&str)` | Parse principal text into a `Principal` |

`Auth` is also available as a plain type (`Auth::new`, `with_principals`,
`add_principal`, `remove_principal`, `list_principals`, `len`) for code that
keeps its own allowlist.

### `http`

**Types:** `HttpRequest`, `HttpResponse` (`new`, `with_streaming_strategy`),
`HttpError` (maps to status codes; `to_response()`), `HttpMethod` (`str::parse`,
`as_str`), `Router`, `StreamingStrategy`, `StreamingCallback`,
`StreamingCallbackToken`, `StreamingCallbackHttpResponse`, the `IntoHttpResponse`
extension trait for `Result<T, HttpError>`, and `http::status::*` constants.

| Function | Description |
|----------|-------------|
| `parse_json<T>(&[u8])` | Parse request body as JSON |
| `to_json<T>(&T)` / `to_json_pretty<T>(&T)` | Serialize to a JSON string |
| `success_response<T>(&T)` | Create 200 JSON response |
| `error_response(u16, &str)` | Create `{"error": ...}` response |
| `json_response(u16, String)` | Create JSON response with status |
| `upgrade_response()` | Ask the gateway to retry as an update call |
| `cors_preflight_response()` | 204 with permissive CORS headers |
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
| `load_candid<T, _>(registry, key)` | Load any CandidType |
| `save_bytes(registry, key, Vec<u8>)` | Save raw bytes |
| `load_bytes(registry, key)` | Load raw bytes |
| `delete(registry, key)` | Delete entry |
| `exists(registry, key)` | Check if key exists (no value copy; uses `StorageRegistry::contains_key`) |
| `size(registry, key)` | Get size in bytes (reads the value) |

`StorageRegistry` is implemented for `StableBTreeMap<String, Vec<u8>, M>`.
Implement it (`insert`, `get`, `remove`, optionally `contains_key`) for other
backends. See [STORAGE_EXAMPLES.md](./STORAGE_EXAMPLES.md) for patterns.

### `large_objects`

All functions take an `owner: Principal` as their first argument (use
`ic_cdk::api::msg_caller()` in endpoints); each owner gets isolated buffers.

| Function | Description |
|----------|-------------|
| `append_chunk(owner, Vec<u8>)` | Add to sequential buffer (checks caps) |
| `load_to_buffer(owner, Vec<u8>)` | Replace the sequential buffer (checks caps) |
| `buffer_size(owner)` | Get sequential buffer size |
| `get_buffer_data(owner)` | Get and clear sequential buffer |
| `clear_buffer(owner)` | Clear sequential buffer |
| `append_parallel_chunk(owner, u32, Vec<u8>)` | Add chunk with ID (checks caps; replaces an existing ID) |
| `remove_parallel_chunk(owner, u32)` | Remove one chunk |
| `parallel_chunk_count(owner)` | Get parallel chunk count |
| `parallel_chunk_ids(owner)` | Sorted chunk IDs present |
| `parallel_buffer_size(owner)` | Total parallel bytes |
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
| `call<T,R>(Principal, &str, T)` | Async call with logging (unbounded wait) |
| `query_call<T,R>(Principal, &str, T)` | Bounded-wait call, for composite queries |
| `call_with_payment<T,R>(Principal, &str, T, u128)` | Call with cycles |
| `call_one_way<T>(Principal, &str, T)` | Fire-and-forget notification |
| `call_no_args<R>(Principal, &str)` | Call with no arguments |

### `telemetry` (feature: `telemetry`)

| Function | Description |
|----------|-------------|
| `init()` / `init_with_principals(Vec<Principal>)` | Initialize telemetry (lazily done on first use if skipped) |
| `collect_metrics()` | Collect canister metrics |
| `update_information()` | Trigger a normal Canistergeek metrics update |
| `get_information(request)` | Canistergeek information query |
| `log_message(msg)` | Log without a level prefix |
| `log_info(msg)` / `log_warning(msg)` / `log_error(msg)` / `log_debug(msg)` | Log with a level prefix |
| `get_canister_log(request)` | Read log entries |
| `is_monitoring_authorized()` | Guard for viewing: controllers, `auth` admins, or monitoring principals |
| `is_monitoring_admin()` | Guard for managing the monitoring list: controllers or `auth` admins |
| `add_monitoring_principal(Principal)` / `remove_monitoring_principal(Principal)` | Manage monitoring access |
| `list_monitoring_principals()` | List monitoring principals |
| `save_to_bytes()` | Serialize metrics, logs, and principals for upgrade |
| `init_from_bytes(Option<Vec<u8>>)` / `init_from_saved(...)` | Restore from saved bytes / decoded parts |

The Canistergeek crate is re-exported as `telemetry::canistergeek_ic_rust`.

## Macros

Each macro defines its endpoints, and any local guard functions it needs, in
the module where it is invoked. Invoke `ic_cdk::export_candid!()` in that same
module.

| Macro | Generates | Arms |
|-------|-----------|------|
| `export_auth_endpoints!()` | `authorize_principal`, `deauthorize_principal`, `get_authorized_principals`, `check_principal_authorized`, `get_authorized_count`, plus a local `is_authorized` guard fn | none |
| `export_telemetry_endpoints!()` | `getCanistergeekInformation`, `updateCanistergeekInformation`, `getCanisterLog`, `authorize_monitoring`, `deauthorize_monitoring`, `get_monitoring_principals` | `(admin_guard = "fn")` to guard the two admin endpoints with your own function (default: `telemetry::is_monitoring_admin`) |
| `generate_upload_endpoints!(...)` | Sequential and parallel upload endpoints plus `get_storage_status` | `()` public (not for production), `(guard = "fn")`, `(guard = "fn", registry = REG)` adds `save_buffer_to_storage`, `save_parallel_to_storage`, `storage_key_exists`, `get_storage_size`, `delete_storage_key` |
| `generate_model_endpoints!(...)` | `setup_model`, `generate`, `reset_generation`, `is_model_loaded`, `get_model_info` | `generate_guard: "fn"` guards `generate`; omit it and inference is public |

## Examples

See the [examples](./examples) directory for complete canister examples.

## Development

CI enforces all of the following; run them before pushing:

```bash
cargo fmt --check
cargo clippy --all-features --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo test --no-default-features
cargo test --features storage,telemetry
cargo test --all-features
cargo check --target wasm32-unknown-unknown --features storage,telemetry
(cd examples/simple_counter/src/example_canister && cargo check --target wasm32-unknown-unknown)
RUSTFLAGS='--cfg getrandom_backend="custom"' cargo check --target wasm32-unknown-unknown --all-features
```

`tests/candid_export.rs` expands every endpoint macro and runs
`ic_cdk::export_candid!` over them; extend it when adding a macro or an
endpoint type. The dummy model, tokenizer, and in-memory registry it uses live
in `tests/fixtures/` and are `include!`d by the doctests too.

## Releasing

1. Add a `## [x.y.z]` entry to `CHANGELOG.md` and set `version` in `Cargo.toml`.
   Under semver for 0.x, breaking changes bump the minor version.
2. Commit, then `cargo publish --dry-run` to confirm the package builds.
3. Push a tag `vx.y.z` on that commit once it is on `main`. The release
   workflow checks the tag matches `Cargo.toml` and is on `main`, reruns the
   native tests and wasm32 checks, and runs `cargo publish` using the
   `CARGO_REGISTRY_TOKEN` repository secret.

## License

MIT OR Apache-2.0