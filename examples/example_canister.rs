// Example canister using ic-dev-kit-rs
// This demonstrates all the major features of the toolkit

use candid::{CandidType, Principal};
use ic_cdk;
use ic_cdk_macros::*;
use ic_dev_kit_rs::prelude::*;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════
//  Initialization
// ═══════════════════════════════════════════════════════════════

#[init]
fn init() {
    // Initialize all modules
    auth::init_with_caller();
    telemetry::init();
    storage::init();

    telemetry::log_info("Canister initialized");
}

// ═══════════════════════════════════════════════════════════════
//  Authentication Examples
// ═══════════════════════════════════════════════════════════════

#[query(guard = "auth::is_authorized")]
fn whoami() -> Principal {
    ic_cdk::api::caller()
}

#[update(guard = "auth::is_authorized")]
fn add_admin(principal: Principal) -> Result<String, String> {
    auth::add_principal(principal)?;
    telemetry::log_info(format!("Added admin: {}", principal));
    Ok("Admin added successfully".to_string())
}

#[query(guard = "auth::is_authorized")]
fn list_admins() -> Vec<Principal> {
    auth::list_principals().unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════
//  HTTP Request Handling
// ═══════════════════════════════════════════════════════════════

#[derive(Serialize, Deserialize, CandidType)]
struct ApiResponse {
    status: String,
    message: String,
    data: Option<String>,
}

#[query]
fn http_request(req: HttpRequest) -> HttpResponse {
    telemetry::track_metrics();

    let path = http::extract_path(&req.url);

    match (req.method.as_str(), path) {
        ("GET", "/") => {
            let response = ApiResponse {
                status: "ok".to_string(),
                message: "Welcome to the example canister".to_string(),
                data: None,
            };
            http::success_response(&response).unwrap_or_else(|e| e.to_response())
        }

        ("GET", "/health") => {
            let response = ApiResponse {
                status: "healthy".to_string(),
                message: "All systems operational".to_string(),
                data: None,
            };
            http::success_response(&response).unwrap_or_else(|e| e.to_response())
        }

        ("GET", "/stats") => {
            let stats = storage::stats();
            http::success_response(&stats).unwrap_or_else(|e| e.to_response())
        }

        _ => http::HttpError::not_found("Endpoint not found").to_response(),
    }
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse {
    telemetry::track_metrics();

    let path = http::extract_path(&req.url);

    match (req.method.as_str(), path) {
        ("POST", "/api/echo") => {
            match http::parse_json::<serde_json::Value>(&req.body) {
                Ok(data) => {
                    telemetry::log_info("Echo request received");
                    http::success_response(&data).unwrap_or_else(|e| e.to_response())
                }
                Err(e) => e.to_response(),
            }
        }

        _ => http::HttpError::not_found("Endpoint not found").to_response(),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Storage Examples
// ═══════════════════════════════════════════════════════════════

#[update(guard = "auth::is_authorized")]
fn upload_file(file_id: String, data: Vec<u8>) -> Result<ObjectMetadata, String> {
    telemetry::track_metrics();
    telemetry::log_info(format!("Uploading file: {}", file_id));

    storage::store(&file_id, data, Some("application/octet-stream".to_string()))
        .map_err(|e| format!("Upload failed: {}", e))
}

#[query]
fn download_file(file_id: String) -> Result<Vec<u8>, String> {
    storage::retrieve(&file_id)
        .map_err(|e| format!("Download failed: {}", e))
}

#[query]
fn list_files() -> Vec<ObjectMetadata> {
    storage::list_with_metadata()
}

#[update(guard = "auth::is_authorized")]
fn delete_file(file_id: String) -> Result<String, String> {
    telemetry::log_info(format!("Deleting file: {}", file_id));

    storage::delete(&file_id)
        .map(|_| "File deleted successfully".to_string())
        .map_err(|e| format!("Delete failed: {}", e))
}

#[query]
fn storage_stats() -> storage::StorageStats {
    storage::stats()
}

// ═══════════════════════════════════════════════════════════════
//  Telemetry Examples
// ═══════════════════════════════════════════════════════════════

#[update(guard = "auth::is_authorized")]
fn add_monitor(principal: Principal) -> Result<String, String> {
    telemetry::add_monitoring_principal(principal)?;
    Ok("Monitor access granted".to_string())
}

#[query(guard = "telemetry::is_monitoring_authorized")]
fn get_canister_metrics() -> canistergeek_ic_rust::api_type::CanisterMetrics {
    telemetry::collect_metrics()
}

// ═══════════════════════════════════════════════════════════════
//  Example Business Logic
// ═══════════════════════════════════════════════════════════════

#[derive(Serialize, Deserialize, CandidType, Clone)]
struct Task {
    id: String,
    title: String,
    completed: bool,
}

#[update]
fn create_task(title: String) -> Result<Task, String> {
    telemetry::track_metrics();
    telemetry::log_info(format!("Creating task: {}", title));

    let task = Task {
        id: format!("task-{}", ic_cdk::api::time()),
        title,
        completed: false,
    };

    // Store task as JSON
    let task_json = serde_json::to_vec(&task)
        .map_err(|e| format!("Serialization error: {}", e))?;

    storage::store(&task.id, task_json, Some("application/json".to_string()))
        .map_err(|e| format!("Storage error: {}", e))?;

    Ok(task)
}

#[query]
fn get_task(task_id: String) -> Result<Task, String> {
    let data = storage::retrieve(&task_id)
        .map_err(|e| format!("Task not found: {}", e))?;

    serde_json::from_slice(&data)
        .map_err(|e| format!("Deserialization error: {}", e))
}

// ═══════════════════════════════════════════════════════════════
//  Upgrade Hooks
// ═══════════════════════════════════════════════════════════════

#[pre_upgrade]
fn pre_upgrade() {
    // Save all module state
    let auth_data = auth::save_to_bytes();
    let telemetry_monitor = telemetry::save_monitor_to_bytes();
    let telemetry_logger = telemetry::save_logger_to_bytes();
    let telemetry_principals = telemetry::save_principals_to_bytes();
    let storage_data = storage::save_to_bytes();

    // In a real canister, you'd save these to stable memory
    // For now, we'll just demonstrate the API
    ic_cdk::println!("Upgrade data prepared");
    ic_cdk::println!("Auth data: {} bytes", auth_data.len());
    ic_cdk::println!("Telemetry monitor: {} bytes", telemetry_monitor.len());
    ic_cdk::println!("Telemetry logger: {} bytes", telemetry_logger.len());
    ic_cdk::println!("Storage data: {} bytes", storage_data.len());
}

#[post_upgrade]
fn post_upgrade() {
    // In a real canister, you'd load from stable memory
    // For now, just reinitialize
    init();
    ic_cdk::println!("Canister upgraded");
}

// Export Candid interface
ic_cdk::export_candid!();