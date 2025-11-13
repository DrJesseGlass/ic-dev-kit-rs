// Telemetry module for Internet Computer canisters with Canistergeek integration
use candid::Principal;
use canistergeek_ic_rust::api_type::*;
use ic_cdk;
use std::cell::RefCell;
use std::collections::HashSet;

// ═══════════════════════════════════════════════════════════════
//  Error Types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Invalid principal")]
    InvalidPrincipal,
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type TelemetryResult<T> = Result<T, TelemetryError>;

// ═══════════════════════════════════════════════════════════════
//  Monitoring Principals Storage
// ═══════════════════════════════════════════════════════════════

pub struct MonitoringAuth {
    principals: RefCell<HashSet<Principal>>,
}

impl MonitoringAuth {
    pub fn new() -> Self {
        Self {
            principals: RefCell::new(HashSet::new()),
        }
    }

    pub fn with_principals(principals: Vec<Principal>) -> Self {
        let mut set = HashSet::new();
        for p in principals {
            set.insert(p);
        }
        Self {
            principals: RefCell::new(set),
        }
    }

    pub fn is_monitoring_authorized(&self, principal: &Principal) -> bool {
        self.principals.borrow().contains(principal)
    }

    pub fn is_controller(&self, principal: &Principal) -> bool {
        ic_cdk::api::is_controller(principal)
    }

    pub fn add_monitoring_principal(&self, principal: Principal) -> TelemetryResult<()> {
        self.principals.borrow_mut().insert(principal);
        Ok(())
    }

    pub fn remove_monitoring_principal(&self, principal: &Principal) -> TelemetryResult<()> {
        self.principals.borrow_mut().remove(principal);
        Ok(())
    }

    pub fn list_monitoring_principals(&self) -> Vec<Principal> {
        self.principals.borrow().iter().cloned().collect()
    }

    pub fn check_access(&self) -> TelemetryResult<()> {
        let caller = ic_cdk::api::caller();

        if self.is_controller(&caller) || self.is_monitoring_authorized(&caller) {
            Ok(())
        } else {
            Err(TelemetryError::Unauthorized)
        }
    }
}

impl Default for MonitoringAuth {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Global State (Thread-Local for IC)
// ═══════════════════════════════════════════════════════════════

thread_local! {
    static AUTH: RefCell<Option<MonitoringAuth>> = RefCell::new(None);
}

// ═══════════════════════════════════════════════════════════════
//  Initialization
// ═══════════════════════════════════════════════════════════════

/// Initialize telemetry system
pub fn init() {
    AUTH.with(|a| {
        *a.borrow_mut() = Some(MonitoringAuth::new());
    });
}

/// Initialize with specific monitoring principals
pub fn init_with_principals(principals: Vec<Principal>) {
    AUTH.with(|a| {
        *a.borrow_mut() = Some(MonitoringAuth::with_principals(principals));
    });
}

/// Initialize from saved state (for post-upgrade)
pub fn init_from_saved(
    monitor_data: Option<canistergeek_ic_rust::monitor::PostUpgradeStableData>,
    logger_data: Option<canistergeek_ic_rust::logger::PostUpgradeStableData>,
    principals: Option<Vec<Principal>>,
) {
    // Initialize monitor
    if let Some(data) = monitor_data {
        canistergeek_ic_rust::monitor::post_upgrade_stable_data(data);
    }

    // Initialize logger
    if let Some(data) = logger_data {
        canistergeek_ic_rust::logger::post_upgrade_stable_data(data);
    }

    // Initialize auth
    AUTH.with(|a| {
        *a.borrow_mut() = Some(
            if let Some(p) = principals {
                MonitoringAuth::with_principals(p)
            } else {
                MonitoringAuth::new()
            }
        );
    });
}

// ═══════════════════════════════════════════════════════════════
//  Helper Functions
// ═══════════════════════════════════════════════════════════════

fn with_auth<R, F>(f: F) -> R
where
    F: FnOnce(&MonitoringAuth) -> R,
{
    AUTH.with(|a| {
        let auth_ref = a.borrow();
        let auth = auth_ref
            .as_ref()
            .expect("Auth not initialized - call telemetry::init() first");
        f(auth)
    })
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Authorization
// ═══════════════════════════════════════════════════════════════

/// Guard function for telemetry endpoints
pub fn is_monitoring_authorized() -> Result<(), String> {
    with_auth(|auth| {
        auth.check_access()
            .map_err(|e| format!("Monitoring authorization failed: {}", e))
    })
}

/// Add a principal to the monitoring allowlist
pub fn add_monitoring_principal(principal: Principal) -> Result<(), String> {
    with_auth(|auth| {
        auth.add_monitoring_principal(principal)
            .map_err(|e| format!("Failed to add monitoring principal: {}", e))
    })
}

/// Remove a principal from the monitoring allowlist
pub fn remove_monitoring_principal(principal: Principal) -> Result<(), String> {
    with_auth(|auth| {
        auth.remove_monitoring_principal(&principal)
            .map_err(|e| format!("Failed to remove monitoring principal: {}", e))
    })
}

/// List all monitoring principals
pub fn list_monitoring_principals() -> Vec<Principal> {
    with_auth(|auth| auth.list_monitoring_principals())
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Monitoring
// ═══════════════════════════════════════════════════════════════

/// Update canister information (call this in update methods)
/// This is the new API method that replaces collect_metrics
pub fn update_information() {
    canistergeek_ic_rust::update_information();
}

/// Get canister information
pub fn get_information(request: GetInformationRequest) -> GetInformationResponse<'static> {
    canistergeek_ic_rust::get_information(request)
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Logging
// ═══════════════════════════════════════════════════════════════

/// Log a message
pub fn log_message(message: impl Into<String>) {
    canistergeek_ic_rust::logger::log_message(message.into());
}

/// Log an info message (convenience wrapper)
pub fn log_info(message: impl Into<String>) {
    let msg = format!("[INFO] {}", message.into());
    canistergeek_ic_rust::logger::log_message(msg);
}

/// Log a warning message (convenience wrapper)
pub fn log_warning(message: impl Into<String>) {
    let msg = format!("[WARN] {}", message.into());
    canistergeek_ic_rust::logger::log_message(msg);
}

/// Log an error message (convenience wrapper)
pub fn log_error(message: impl Into<String>) {
    let msg = format!("[ERROR] {}", message.into());
    canistergeek_ic_rust::logger::log_message(msg);
}

/// Log a debug message (convenience wrapper)
pub fn log_debug(message: impl Into<String>) {
    let msg = format!("[DEBUG] {}", message.into());
    canistergeek_ic_rust::logger::log_message(msg);
}

/// Get canister log
pub fn get_canister_log(request: CanisterLogRequest) -> Option<CanisterLogResponse<'static>> {
    canistergeek_ic_rust::logger::get_canister_log(request)
}

// ═══════════════════════════════════════════════════════════════
//  Persistence (for upgrade)
// ═══════════════════════════════════════════════════════════════

/// Get monitor stable data for pre-upgrade
pub fn pre_upgrade_monitor_data() -> canistergeek_ic_rust::monitor::PostUpgradeStableData {
    canistergeek_ic_rust::monitor::pre_upgrade_stable_data()
}

/// Get logger stable data for pre-upgrade
pub fn pre_upgrade_logger_data() -> canistergeek_ic_rust::logger::PostUpgradeStableData {
    canistergeek_ic_rust::logger::pre_upgrade_stable_data()
}

/// Save monitoring principals to bytes
pub fn save_principals_to_bytes() -> Vec<u8> {
    let principals = list_monitoring_principals();
    candid::encode_args((&principals,)).unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════
//  IC CDK Exported Functions (Optional)
// ═══════════════════════════════════════════════════════════════

/// Query to get canister information (guarded)
#[ic_cdk::query(guard = "is_monitoring_authorized")]
pub fn get_canistergeek_information(request: GetInformationRequest) -> GetInformationResponse<'static> {
    get_information(request)
}

/// Update to update canister information (guarded)
#[ic_cdk::update(guard = "is_monitoring_authorized")]
pub fn update_canistergeek_information(request: UpdateInformationRequest) {
    canistergeek_ic_rust::update_information_with_request(request);
}

/// Query to get canister log (guarded)
#[ic_cdk::query(guard = "is_monitoring_authorized")]
pub fn get_canister_log_query(request: CanisterLogRequest) -> Option<CanisterLogResponse<'static>> {
    get_canister_log(request)
}

/// Update to add monitoring principal (requires controller or monitoring access)
#[ic_cdk::update(guard = "is_monitoring_authorized")]
pub fn authorize_monitoring(principal: Principal) {
    let _ = add_monitoring_principal(principal);
}

/// Update to remove monitoring principal (requires controller or monitoring access)
#[ic_cdk::update(guard = "is_monitoring_authorized")]
pub fn deauthorize_monitoring(principal: Principal) {
    let _ = remove_monitoring_principal(principal);
}

/// Query to list monitoring principals (guarded)
#[ic_cdk::query(guard = "is_monitoring_authorized")]
pub fn get_monitoring_principals() -> Vec<Principal> {
    list_monitoring_principals()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_auth() {
        let auth = MonitoringAuth::new();
        let test_principal = Principal::anonymous();

        // Initially not authorized
        assert!(!auth.is_monitoring_authorized(&test_principal));

        // Add principal
        auth.add_monitoring_principal(test_principal).unwrap();
        assert!(auth.is_monitoring_authorized(&test_principal));

        // List principals
        let list = auth.list_monitoring_principals();
        assert_eq!(list.len(), 1);
        assert!(list.contains(&test_principal));

        // Remove principal
        auth.remove_monitoring_principal(&test_principal).unwrap();
        assert!(!auth.is_monitoring_authorized(&test_principal));
    }
}