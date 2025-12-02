// Telemetry module for Internet Computer canisters with Canistergeek integration

#![cfg(feature = "telemetry")]

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

    pub fn add_monitoring_principal(&self, principal: Principal) {
        self.principals.borrow_mut().insert(principal);
    }

    pub fn remove_monitoring_principal(&self, principal: &Principal) {
        self.principals.borrow_mut().remove(principal);
    }

    pub fn list_monitoring_principals(&self) -> Vec<Principal> {
        self.principals.borrow().iter().cloned().collect()
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
    static MONITORING_AUTH: RefCell<Option<MonitoringAuth>> = RefCell::new(None);
}

// ═══════════════════════════════════════════════════════════════
//  Initialization
// ═══════════════════════════════════════════════════════════════

/// Initialize telemetry system
pub fn init() {
    MONITORING_AUTH.with(|a| {
        *a.borrow_mut() = Some(MonitoringAuth::new());
    });
}

/// Initialize with specific monitoring principals
pub fn init_with_principals(principals: Vec<Principal>) {
    MONITORING_AUTH.with(|a| {
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
    MONITORING_AUTH.with(|a| {
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

fn with_monitoring_auth<R, F>(f: F) -> R
where
    F: FnOnce(&MonitoringAuth) -> R,
{
    MONITORING_AUTH.with(|a| {
        let auth_ref = a.borrow();
        let auth = auth_ref
            .as_ref()
            .expect("Telemetry not initialized - call telemetry::init() first");
        f(auth)
    })
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Authorization
// ═══════════════════════════════════════════════════════════════

/// Guard function for telemetry viewing endpoints
/// Allows: monitoring principals, admins, or controllers
pub fn is_monitoring_authorized() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();

    // Check if controller
    if ic_cdk::api::is_controller(&caller) {
        return Ok(());
    }

    // Check if admin (from auth module)
    if crate::auth::is_authorized().is_ok() {
        return Ok(());
    }

    // Check if monitoring principal
    let is_monitoring = with_monitoring_auth(|auth| auth.is_monitoring_authorized(&caller));
    if is_monitoring {
        return Ok(());
    }

    Err("Monitoring authorization failed: caller is not a controller, admin, or monitoring principal".to_string())
}

/// Add a principal to the monitoring allowlist (requires admin)
pub fn add_monitoring_principal(principal: Principal) {
    with_monitoring_auth(|auth| auth.add_monitoring_principal(principal));
}

/// Remove a principal from the monitoring allowlist (requires admin)
pub fn remove_monitoring_principal(principal: Principal) {
    with_monitoring_auth(|auth| auth.remove_monitoring_principal(&principal));
}

/// List all monitoring principals
pub fn list_monitoring_principals() -> Vec<Principal> {
    with_monitoring_auth(|auth| auth.list_monitoring_principals())
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Monitoring
// ═══════════════════════════════════════════════════════════════

pub fn update_information() {
    let request = UpdateInformationRequest {
        metrics: Some(CollectMetricsRequestType::normal),
    };
    canistergeek_ic_rust::update_information(request);
}

pub fn collect_metrics() {
    canistergeek_ic_rust::monitor::collect_metrics();
}

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
    canistergeek_ic_rust::logger::get_canister_log(Some(request))
}

// ═══════════════════════════════════════════════════════════════
//  Persistence (for upgrade)
// ═══════════════════════════════════════════════════════════════

/// Save all telemetry state to bytes (for pre_upgrade)
pub fn save_to_bytes() -> Vec<u8> {
    let monitor_data = canistergeek_ic_rust::monitor::pre_upgrade_stable_data();
    let logger_data = canistergeek_ic_rust::logger::pre_upgrade_stable_data();
    let principals = list_monitoring_principals();

    candid::encode_args((monitor_data, logger_data, principals)).unwrap_or_default()
}

/// Initialize telemetry from saved bytes (for post_upgrade)
pub fn init_from_bytes(bytes: Option<Vec<u8>>) {
    if let Some(data) = bytes {
        if let Ok((monitor_data, logger_data, principals)) = candid::decode_args::<(
            canistergeek_ic_rust::monitor::PostUpgradeStableData,
            canistergeek_ic_rust::logger::PostUpgradeStableData,
            Vec<Principal>,
        )>(&data) {
            init_from_saved(Some(monitor_data), Some(logger_data), Some(principals));
            return;
        }
    }
    // Fallback to fresh init if restore fails
    init();
}

/// Save monitoring principals to bytes (legacy, kept for compatibility)
pub fn save_principals_to_bytes() -> Vec<u8> {
    let principals = list_monitoring_principals();
    candid::encode_args((&principals,)).unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════
//  Macro for Exporting Telemetry Endpoints
// ═══════════════════════════════════════════════════════════════

#[macro_export]
macro_rules! export_telemetry_endpoints {
    () => {
        fn is_monitoring_authorized() -> Result<(), String> {
            $crate::telemetry::is_monitoring_authorized()
        }

        #[ic_cdk::query(name = "getCanistergeekInformation", guard = "is_monitoring_authorized")]
        fn get_canistergeek_information(
            request: canistergeek_ic_rust::api_type::GetInformationRequest
        ) -> canistergeek_ic_rust::api_type::GetInformationResponse<'static> {
            $crate::telemetry::get_information(request)
        }

        #[ic_cdk::update(name = "updateCanistergeekInformation", guard = "is_monitoring_authorized")]
        fn update_canistergeek_information(
            request: canistergeek_ic_rust::api_type::UpdateInformationRequest
        ) {
            canistergeek_ic_rust::update_information(request);
        }

        #[ic_cdk::query(name = "getCanisterLog", guard = "is_monitoring_authorized")]
        fn get_canister_log_messages(
            request: canistergeek_ic_rust::api_type::CanisterLogRequest
        ) -> Option<canistergeek_ic_rust::api_type::CanisterLogResponse<'static>> {
            $crate::telemetry::get_canister_log(request)
        }

        // Keep monitoring auth endpoints in snake_case (our own API)
        #[ic_cdk::update(guard = "is_authorized")]
        fn authorize_monitoring(principal: candid::Principal) {
            $crate::telemetry::add_monitoring_principal(principal);
        }

        #[ic_cdk::update(guard = "is_authorized")]
        fn deauthorize_monitoring(principal: candid::Principal) {
            $crate::telemetry::remove_monitoring_principal(principal);
        }

        #[ic_cdk::query(guard = "is_monitoring_authorized")]
        fn get_monitoring_principals() -> Vec<candid::Principal> {
            $crate::telemetry::list_monitoring_principals()
        }
    };
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