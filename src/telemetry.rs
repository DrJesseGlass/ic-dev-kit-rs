//! Telemetry module with Canistergeek integration.
//!
//! Provides monitoring metrics and logging for IC canisters using Canistergeek.
//! Requires the `telemetry` feature.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::telemetry;
//!
//! #[ic_cdk::init]
//! fn init() {
//!     telemetry::init();
//! }
//!
//! #[ic_cdk::update]
//! fn my_function() {
//!     telemetry::collect_metrics();
//!     telemetry::log_info("Function called");
//!     // ...
//! }
//! ```
//!
//! # Upgrade Persistence
//!
//! ```rust,ignore
//! #[ic_cdk::pre_upgrade]
//! fn pre_upgrade() {
//!     let bytes = telemetry::save_to_bytes();
//!     // Store bytes in stable memory
//! }
//!
//! #[ic_cdk::post_upgrade]
//! fn post_upgrade() {
//!     // Load bytes from stable memory
//!     telemetry::init_from_bytes(Some(bytes));
//! }
//! ```

#![cfg(feature = "telemetry")]

/// Re-export of the underlying Canistergeek crate.
///
/// [`export_telemetry_endpoints!`](crate::export_telemetry_endpoints) refers
/// to its types through this path, so consumers do not need a direct
/// dependency on `canistergeek_ic_rust`.
pub use canistergeek_ic_rust;

use crate::auth::Auth;
use candid::Principal;
use canistergeek_ic_rust::api_type::*;
use std::cell::RefCell;

// ═══════════════════════════════════════════════════════════════
//  Global State (Thread-Local for IC)
// ═══════════════════════════════════════════════════════════════

thread_local! {
    /// Principals allowed to *view* monitoring data. Kept separate from the
    /// main [`auth`](crate::auth) allowlist so read-only observers do not
    /// become admins. Reuses [`Auth`] as the set type.
    static MONITORING_AUTH: RefCell<Option<Auth>> = RefCell::new(None);
}

// ═══════════════════════════════════════════════════════════════
//  Initialization
// ═══════════════════════════════════════════════════════════════

/// Initialize the telemetry system.
///
/// Call this in your `#[ic_cdk::init]` function.
pub fn init() {
    init_with_principals(Vec::new());
}

/// Initialize with specific monitoring principals.
pub fn init_with_principals(principals: Vec<Principal>) {
    MONITORING_AUTH.with(|a| *a.borrow_mut() = Some(Auth::with_principals(principals)));
}

/// Initialize from saved state (for post-upgrade).
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

    init_with_principals(principals.unwrap_or_default());
}

// ═══════════════════════════════════════════════════════════════
//  Helper Functions
// ═══════════════════════════════════════════════════════════════

/// Run `f` against the monitoring auth, lazily initializing an empty one if
/// [`init`] was never called. This keeps telemetry usable standalone and
/// guards from trapping the canister.
fn with_monitoring_auth<R, F>(f: F) -> R
where
    F: FnOnce(&Auth) -> R,
{
    MONITORING_AUTH.with(|a| f(a.borrow_mut().get_or_insert_with(Auth::new)))
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Authorization
// ═══════════════════════════════════════════════════════════════

/// Guard function for telemetry viewing endpoints.
///
/// Allows access to: controllers, admins (via auth module), or monitoring principals.
///
/// # Example
///
/// ```rust,ignore
/// #[ic_cdk::query(guard = "telemetry::is_monitoring_authorized")]
/// fn get_logs() -> Vec<String> {
///     // ...
/// }
/// ```
pub fn is_monitoring_authorized() -> Result<(), String> {
    // Admins (controllers + auth-module admins) can always view.
    if is_monitoring_admin().is_ok() {
        return Ok(());
    }

    // Otherwise the caller must be on the monitoring allowlist.
    let caller = ic_cdk::api::msg_caller();
    if with_monitoring_auth(|auth| auth.is_authorized(&caller)) {
        return Ok(());
    }

    Err("Monitoring authorization failed: caller is not a controller, admin, or monitoring principal".to_string())
}

/// Guard function for telemetry *administration* endpoints (managing the
/// monitoring allowlist).
///
/// Allows canister controllers always, plus admins from the [`auth`](crate::auth)
/// module when that module has been initialized. Does not trap if auth was
/// never initialized — controllers can still manage monitoring access.
pub fn is_monitoring_admin() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();

    if ic_cdk::api::is_controller(&caller) {
        return Ok(());
    }

    if crate::auth::is_authorized().is_ok() {
        return Ok(());
    }

    Err("Monitoring admin authorization failed: caller is not a controller or authorized admin".to_string())
}

/// Add a principal to the monitoring allowlist.
///
/// Requires admin authorization.
pub fn add_monitoring_principal(principal: Principal) {
    with_monitoring_auth(|auth| auth.add_principal(principal));
}

/// Remove a principal from the monitoring allowlist.
///
/// Requires admin authorization. Unlike [`auth::remove_principal`]
/// (crate::auth::remove_principal), emptying this list is allowed: controllers
/// and admins can always view monitoring data.
pub fn remove_monitoring_principal(principal: Principal) {
    with_monitoring_auth(|auth| auth.remove_principal(&principal));
}

/// List all monitoring principals.
pub fn list_monitoring_principals() -> Vec<Principal> {
    with_monitoring_auth(|auth| auth.list_principals())
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Monitoring
// ═══════════════════════════════════════════════════════════════

/// Update Canistergeek information.
pub fn update_information() {
    let request = UpdateInformationRequest {
        metrics: Some(CollectMetricsRequestType::normal),
    };
    canistergeek_ic_rust::update_information(request);
}

/// Collect canister metrics.
///
/// Call this at the start of update/query methods you want to track.
pub fn collect_metrics() {
    canistergeek_ic_rust::monitor::collect_metrics();
}

/// Get Canistergeek information.
pub fn get_information(request: GetInformationRequest) -> GetInformationResponse {
    canistergeek_ic_rust::get_information(request)
}

// ═══════════════════════════════════════════════════════════════
//  Public API - Logging
// ═══════════════════════════════════════════════════════════════

/// Log a message to Canistergeek.
pub fn log_message(message: impl Into<String>) {
    canistergeek_ic_rust::logger::log_message(message.into());
}

fn log_with_level(level: &str, message: impl Into<String>) {
    log_message(format!("[{}] {}", level, message.into()));
}

/// Log an info message, prefixed with `[INFO]`.
pub fn log_info(message: impl Into<String>) {
    log_with_level("INFO", message);
}

/// Log a warning message, prefixed with `[WARN]`.
pub fn log_warning(message: impl Into<String>) {
    log_with_level("WARN", message);
}

/// Log an error message, prefixed with `[ERROR]`.
pub fn log_error(message: impl Into<String>) {
    log_with_level("ERROR", message);
}

/// Log a debug message, prefixed with `[DEBUG]`.
pub fn log_debug(message: impl Into<String>) {
    log_with_level("DEBUG", message);
}

/// Get canister log entries.
pub fn get_canister_log(request: CanisterLogRequest) -> Option<CanisterLogResponse> {
    canistergeek_ic_rust::logger::get_canister_log(Some(request))
}

// ═══════════════════════════════════════════════════════════════
//  Persistence (for upgrade)
// ═══════════════════════════════════════════════════════════════

/// Save all telemetry state to bytes (for pre_upgrade).
///
/// Includes monitor data, logger data, and monitoring principals.
pub fn save_to_bytes() -> Vec<u8> {
    let monitor_data = canistergeek_ic_rust::monitor::pre_upgrade_stable_data();
    let logger_data = canistergeek_ic_rust::logger::pre_upgrade_stable_data();
    let principals = list_monitoring_principals();

    candid::encode_args((monitor_data, logger_data, principals)).unwrap_or_default()
}

/// Initialize telemetry from saved bytes (for post_upgrade).
///
/// Falls back to fresh initialization if deserialization fails.
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

// ═══════════════════════════════════════════════════════════════
//  Macro for Exporting Telemetry Endpoints
// ═══════════════════════════════════════════════════════════════

/// Generate standard Canistergeek-compatible telemetry endpoints.
///
/// This macro creates the following IC endpoints:
/// - `getCanistergeekInformation` - Get Canistergeek metrics (guarded)
/// - `updateCanistergeekInformation` - Update metrics (guarded)
/// - `getCanisterLog` - Get log messages (guarded)
/// - `authorize_monitoring` - Add monitoring principal (controllers/admins)
/// - `deauthorize_monitoring` - Remove monitoring principal (controllers/admins)
/// - `get_monitoring_principals` - List monitoring principals (guarded)
///
/// The macro is self-contained: by default, administration endpoints are
/// guarded by [`telemetry::is_monitoring_admin`](crate::telemetry::is_monitoring_admin)
/// (controllers always allowed, plus `auth` module admins when initialized),
/// so it can be used with or without [`export_auth_endpoints!`](crate::export_auth_endpoints)
/// and in any order. Pass `admin_guard = "my_guard"` to use your own guard
/// function for the administration endpoints instead.
///
/// The Canistergeek request/response types are referenced through
/// [`telemetry::canistergeek_ic_rust`](crate::telemetry::canistergeek_ic_rust),
/// so consumers do not need their own dependency on that crate. Invoke
/// `ic_cdk::export_candid!()` in the same module as this macro.
///
/// # Example
///
/// ```rust,ignore
/// ic_dev_kit_rs::export_telemetry_endpoints!();
/// // or, with a custom admin guard:
/// ic_dev_kit_rs::export_telemetry_endpoints!(admin_guard = "my_admin_guard");
/// ```
#[macro_export]
macro_rules! export_telemetry_endpoints {
    () => {
        fn is_monitoring_admin() -> Result<(), String> {
            $crate::telemetry::is_monitoring_admin()
        }

        $crate::export_telemetry_endpoints!(admin_guard = "is_monitoring_admin");
    };

    (admin_guard = $admin_guard:expr) => {
        fn is_monitoring_authorized() -> Result<(), String> {
            $crate::telemetry::is_monitoring_authorized()
        }

        // `ic_cdk::export_candid!` re-parses stringified endpoint signatures,
        // and `$crate` is not parseable there, so the Canistergeek types are
        // bound to local aliases first. Invoke `export_candid!` in the same
        // module as this macro so the aliases are in scope.
        type __CgGetInformationRequest =
            $crate::telemetry::canistergeek_ic_rust::api_type::GetInformationRequest;
        type __CgGetInformationResponse =
            $crate::telemetry::canistergeek_ic_rust::api_type::GetInformationResponse;
        type __CgUpdateInformationRequest =
            $crate::telemetry::canistergeek_ic_rust::api_type::UpdateInformationRequest;
        type __CgCanisterLogRequest =
            $crate::telemetry::canistergeek_ic_rust::api_type::CanisterLogRequest;
        type __CgCanisterLogResponse =
            $crate::telemetry::canistergeek_ic_rust::api_type::CanisterLogResponse;

        #[ic_cdk::query(name = "getCanistergeekInformation", guard = "is_monitoring_authorized")]
        fn get_canistergeek_information(
            request: __CgGetInformationRequest,
        ) -> __CgGetInformationResponse {
            $crate::telemetry::get_information(request)
        }

        #[ic_cdk::update(name = "updateCanistergeekInformation", guard = "is_monitoring_authorized")]
        fn update_canistergeek_information(request: __CgUpdateInformationRequest) {
            $crate::telemetry::canistergeek_ic_rust::update_information(request);
        }

        #[ic_cdk::query(name = "getCanisterLog", guard = "is_monitoring_authorized")]
        fn get_canister_log_messages(
            request: __CgCanisterLogRequest,
        ) -> Option<__CgCanisterLogResponse> {
            $crate::telemetry::get_canister_log(request)
        }

        // Keep monitoring auth endpoints in snake_case (our own API)
        #[ic_cdk::update(guard = $admin_guard)]
        fn authorize_monitoring(principal: ::candid::Principal) {
            $crate::telemetry::add_monitoring_principal(principal);
        }

        #[ic_cdk::update(guard = $admin_guard)]
        fn deauthorize_monitoring(principal: ::candid::Principal) {
            $crate::telemetry::remove_monitoring_principal(principal);
        }

        #[ic_cdk::query(guard = "is_monitoring_authorized")]
        fn get_monitoring_principals() -> Vec<::candid::Principal> {
            $crate::telemetry::list_monitoring_principals()
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_allowlist_lazily_initializes_and_may_be_emptied() {
        // Fresh test thread: MONITORING_AUTH is None and must not trap.
        let p = Principal::anonymous();
        assert!(list_monitoring_principals().is_empty());

        add_monitoring_principal(p);
        assert_eq!(list_monitoring_principals(), vec![p]);

        // Emptying the monitoring list is allowed (controllers still see data).
        remove_monitoring_principal(p);
        assert!(list_monitoring_principals().is_empty());
    }

    #[test]
    fn test_init_replaces_allowlist() {
        let (a, b) = (Principal::anonymous(), Principal::from_slice(&[1]));
        init_with_principals(vec![a]);
        init_from_saved(None, None, Some(vec![b]));
        assert_eq!(list_monitoring_principals(), vec![b]);
        init();
        assert!(list_monitoring_principals().is_empty());
    }
}
