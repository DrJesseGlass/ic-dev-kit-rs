// Telemetry module for Internet Computer canisters with Canistergeek integration
use candid::Principal;
use canistergeek_ic_rust::api_type::*;
use canistergeek_ic_rust::{monitor::*, logger::*};
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
    static MONITOR: RefCell<Option<Monitor>> = RefCell::new(None);
    static LOGGER: RefCell<Option<Logger<CustomLogEntry>>> = RefCell::new(None);
    static AUTH: RefCell<Option<MonitoringAuth>> = RefCell::new(None);
}

// Custom log entry type - extend as needed
#[derive(Clone, Debug)]
pub struct CustomLogEntry {
    pub timestamp: u64,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Clone, Debug)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl LogEntry for CustomLogEntry {
    fn timestamp(&self) -> i64 {
        self.timestamp as i64
    }
}

// ═══════════════════════════════════════════════════════════════
//  Initialization
// ═══════════════════════════════════════════════════════════════

/// Initialize telemetry system
pub fn init() {
    MONITOR.with(|m| {
        *m.borrow_mut() = Some(Monitor::init());
    });

    LOGGER.with(|l| {
        *l.borrow_mut() = Some(Logger::init(1000, 3000).expect("Failed to initialize logger"));
    });

    AUTH.with(|a| {
        *a.borrow_mut() = Some(MonitoringAuth::new());
    });
}

/// Initialize with specific monitoring principals
pub fn init_with_principals(principals: Vec<Principal>) {
    MONITOR.with(|m| {
        *m.borrow_mut() = Some(Monitor::init());
    });

    LOGGER.with(|l| {
        *l.borrow_mut() = Some(Logger::init(1000, 3000).expect("Failed to initialize logger"));
    });

    AUTH.with(|a| {
        *a.borrow_mut() = Some(MonitoringAuth::with_principals(principals));
    });
}

/// Initialize from saved state (for post-upgrade)
pub fn init_from_saved(
    monitor_data: Option<Vec<u8>>,
    logger_data: Option<Vec<u8>>,
    principals: Option<Vec<Principal>>,
) {
    // Initialize monitor
    MONITOR.with(|m| {
        if let Some(data) = monitor_data {
            if let Ok(decoded) = candid::decode_args::<(MonitorData,)>(&data) {
                *m.borrow_mut() = Some(Monitor::init_with_data(decoded.0));
            } else {
                *m.borrow_mut() = Some(Monitor::init());
            }
        } else {
            *m.borrow_mut() = Some(Monitor::init());
        }
    });

    // Initialize logger
    LOGGER.with(|l| {
        if let Some(data) = logger_data {
            if let Ok(decoded) = candid::decode_args::<(LoggerData<CustomLogEntry>,)>(&data) {
                *l.borrow_mut() = Some(Logger::init_with_data(1000, 3000, decoded.0).expect("Failed to initialize logger"));
            } else {
                *l.borrow_mut() = Some(Logger::init(1000, 3000).expect("Failed to initialize logger"));
            }
        } else {
            *l.borrow_mut() = Some(Logger::init(1000, 3000).expect("Failed to initialize logger"));
        }
    });

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

fn with_monitor<R, F>(f: F) -> R
where
    F: FnOnce(&Monitor) -> R,
{
    MONITOR.with(|m| {
        let monitor_ref = m.borrow();
        let monitor = monitor_ref
            .as_ref()
            .expect("Monitor not initialized - call telemetry::init() first");
        f(monitor)
    })
}

fn with_logger<R, F>(f: F) -> R
where
    F: FnOnce(&mut Logger<CustomLogEntry>) -> R,
{
    LOGGER.with(|l| {
        let mut logger_ref = l.borrow_mut();
        let logger = logger_ref
            .as_mut()
            .expect("Logger not initialized - call telemetry::init() first");
        f(logger)
    })
}

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

/// Track canister metrics (call this in update methods)
pub fn track_metrics() {
    with_monitor(|monitor| {
        monitor.track_metrics();
    });
}

/// Collect canister metrics
pub fn collect_metrics() -> CanisterMetrics {
    with_monitor(|monitor| monitor.collect_metrics())
}

/// Get canister information
pub fn get_information(request: GetInformationRequest) -> GetInformationResponse {
    with_monitor(|monitor| monitor.get_information(request))
}

/// Get latest metrics
pub fn get_latest_metrics() -> Option<CanisterMetrics> {
    with_monitor(|monitor| monitor.get_latest_metrics())
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Logging
// ═══════════════════════════════════════════════════════════════

/// Log an info message
pub fn log_info(message: impl Into<String>) {
    let entry = CustomLogEntry {
        timestamp: ic_cdk::api::time(),
        message: message.into(),
        level: LogLevel::Info,
    };
    with_logger(|logger| {
        logger.log_message(entry);
    });
}

/// Log a warning message
pub fn log_warning(message: impl Into<String>) {
    let entry = CustomLogEntry {
        timestamp: ic_cdk::api::time(),
        message: message.into(),
        level: LogLevel::Warning,
    };
    with_logger(|logger| {
        logger.log_message(entry);
    });
}

/// Log an error message
pub fn log_error(message: impl Into<String>) {
    let entry = CustomLogEntry {
        timestamp: ic_cdk::api::time(),
        message: message.into(),
        level: LogLevel::Error,
    };
    with_logger(|logger| {
        logger.log_message(entry);
    });
}

/// Log a debug message
pub fn log_debug(message: impl Into<String>) {
    let entry = CustomLogEntry {
        timestamp: ic_cdk::api::time(),
        message: message.into(),
        level: LogLevel::Debug,
    };
    with_logger(|logger| {
        logger.log_message(entry);
    });
}

/// Get log messages
pub fn get_log_messages(request: GetLogMessagesRequest) -> GetLogMessagesResponse<CustomLogEntry> {
    with_logger(|logger| logger.get_log_messages(request))
}

// ═══════════════════════════════════════════════════════════════
//  Persistence (for upgrade)
// ═══════════════════════════════════════════════════════════════

/// Save monitor data to bytes
pub fn save_monitor_to_bytes() -> Vec<u8> {
    with_monitor(|monitor| {
        let data = monitor.get_data();
        candid::encode_args((data,)).unwrap_or_default()
    })
}

/// Save logger data to bytes
pub fn save_logger_to_bytes() -> Vec<u8> {
    with_logger(|logger| {
        let data = logger.get_data();
        candid::encode_args((data,)).unwrap_or_default()
    })
}

/// Save monitoring principals to bytes
pub fn save_principals_to_bytes() -> Vec<u8> {
    let principals = list_monitoring_principals();
    candid::encode_args((&principals,)).unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════
//  IC CDK Exported Functions (Optional)
// ═══════════════════════════════════════════════════════════════

/// Query to get canister metrics (guarded)
#[ic_cdk::query(guard = "is_monitoring_authorized")]
pub fn get_canister_metrics() -> CanisterMetrics {
    collect_metrics()
}

/// Query to get canister information (guarded)
#[ic_cdk::query(guard = "is_monitoring_authorized")]
pub fn get_canister_information(request: GetInformationRequest) -> GetInformationResponse {
    get_information(request)
}

/// Query to get log messages (guarded)
#[ic_cdk::query(guard = "is_monitoring_authorized")]
pub fn get_canister_log(request: GetLogMessagesRequest) -> GetLogMessagesResponse<CustomLogEntry> {
    get_log_messages(request)
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

    #[test]
    fn test_log_levels() {
        let info = CustomLogEntry {
            timestamp: 123456,
            message: "Info message".to_string(),
            level: LogLevel::Info,
        };
        assert_eq!(info.timestamp(), 123456);
    }
}
