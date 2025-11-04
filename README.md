# ic-dev-kit-rs

Rust toolkit for IC development - standardizes HTTP generation, large object storage, Canister Geek integration, and common canister patterns.

## Features

- **Authentication**: Principal-based authorization with guard functions
- **HTTP Handling**: Request/response types, routing, JSON utilities
- **Telemetry**: Canistergeek integration for monitoring and logging
- **Storage**: Large object storage with automatic chunking

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

### 4. Large Object Storage

```rust
use ic_dev_kit_rs::storage;

#[ic_cdk::init]
fn init() {
    storage::init(); // 1MB chunk size by default
}

#[ic_cdk::update]
fn upload_file(file_id: String, data: Vec<u8>) -> Result<String, String> {
    storage::store(&file_id, data, Some("application/octet-stream".to_string()))
        .map(|_| "File uploaded successfully".to_string())
        .map_err(|e| format!("Upload failed: {}", e))
}

#[ic_cdk::query]
fn download_file(file_id: String) -> Result<Vec<u8>, String> {
    storage::retrieve(&file_id)
        .map_err(|e| format!("Download failed: {}", e))
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

- `store()` - Store large object (auto-chunking)
- `retrieve()` - Retrieve complete object
- `init_chunked_upload()` - Start chunked upload
- `store_chunk()` - Store individual chunk
- `retrieve_chunk()` - Get specific chunk
- `delete()` - Delete object
- `list_objects()` - List all object IDs
- `get_metadata()` - Get object metadata
- `stats()` - Storage statistics

## Examples

See the [examples](./examples) directory for complete canister examples.

## Contributing

Contributions welcome! Please feel free to submit a Pull Request.

## License

MIT OR Apache-2.0