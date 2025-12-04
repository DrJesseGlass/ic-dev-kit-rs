//! Inter-canister call wrappers with automatic logging.
//!
//! Provides convenient wrappers around IC inter-canister calls with:
//! - Automatic logging of call start/success/failure
//! - Consistent error formatting
//! - Support for calls with cycles
//! - One-way notifications
//!
//! # Example
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::intercanister;
//! use candid::Principal;
//!
//! #[ic_cdk::update]
//! async fn call_other(canister_id: Principal) -> Result<String, String> {
//!     intercanister::call(canister_id, "get_data", ()).await
//! }
//! ```
//!
//! # Note on ic-cdk 0.19
//!
//! The ic-cdk 0.19 crate is in a transitional state where types have moved
//! to `ic_cdk::call` but functions are still in `ic_cdk::api::call` (deprecated).
//! This module uses `#[allow(deprecated)]` until the API is fully updated.

use candid::{CandidType, Principal};
use serde::de::DeserializeOwned;

// ═══════════════════════════════════════════════════════════════
//  Core Call Functions
// ═══════════════════════════════════════════════════════════════

/// Make an inter-canister call with automatic logging.
///
/// # Type Parameters
///
/// * `T` - The argument type (must implement `ArgumentEncoder`)
/// * `R` - The return type (must implement `DeserializeOwned + CandidType`)
///
/// # Arguments
///
/// * `canister_id` - The target canister's principal
/// * `method` - The method name to call
/// * `args` - The arguments to pass
///
/// # Example
///
/// ```rust,ignore
/// let result: MyResponse = intercanister::call(
///     canister_id,
///     "my_method",
///     (arg1, arg2)
/// ).await?;
/// ```
#[allow(deprecated)]
pub async fn call<T, R>(canister_id: Principal, method: &str, args: T) -> Result<R, String>
where
    T: candid::utils::ArgumentEncoder,
    R: DeserializeOwned + CandidType,
{
    log_call_start(canister_id, method);

    let result: Result<(R,), _> = ic_cdk::api::call::call(canister_id, method, args).await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map(|r| r.0)
        .map_err(|e| format_call_error(canister_id, method, e))
}

/// Make an inter-canister call with cycles attached.
///
/// # Arguments
///
/// * `canister_id` - The target canister's principal
/// * `method` - The method name to call
/// * `args` - The arguments to pass
/// * `cycles` - The number of cycles to attach
///
/// # Example
///
/// ```rust,ignore
/// let result: MyResponse = intercanister::call_with_payment(
///     canister_id,
///     "paid_method",
///     (arg1,),
///     1_000_000  // 1M cycles
/// ).await?;
/// ```
#[allow(deprecated)]
pub async fn call_with_payment<T, R>(
    canister_id: Principal,
    method: &str,
    args: T,
    cycles: u128,
) -> Result<R, String>
where
    T: candid::utils::ArgumentEncoder,
    R: DeserializeOwned + CandidType,
{
    log_call_start_with_cycles(canister_id, method, cycles);

    let result: Result<(R,), _> =
        ic_cdk::api::call::call_with_payment128(canister_id, method, args, cycles).await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map(|r| r.0)
        .map_err(|e| format_call_error(canister_id, method, e))
}

/// Make a one-way inter-canister call (fire-and-forget).
///
/// This sends a notification to another canister without waiting for a response.
/// Use this when you don't need to know if the call succeeded.
///
/// # Arguments
///
/// * `canister_id` - The target canister's principal
/// * `method` - The method name to call
/// * `args` - The arguments to pass
///
/// # Example
///
/// ```rust,ignore
/// intercanister::call_one_way(
///     logger_canister,
///     "log_event",
///     ("user_action", user_id)
/// )?;
/// ```
#[allow(deprecated)]
pub fn call_one_way<T>(canister_id: Principal, method: &str, args: T) -> Result<(), String>
where
    T: candid::utils::ArgumentEncoder,
{
    log_call_start(canister_id, method);

    let result: Result<(), _> = ic_cdk::api::call::notify(canister_id, method, args);

    match &result {
        Ok(_) => {
            log_call_success(canister_id, method);
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Notify failed: {:?}", e);
            log_message(&err_msg);
            Err(err_msg)
        }
    }
}

/// Convenience function to call a method that takes no arguments.
///
/// # Example
///
/// ```rust,ignore
/// let count: u64 = intercanister::call_no_args(canister_id, "get_count").await?;
/// ```
pub async fn call_no_args<R>(canister_id: Principal, method: &str) -> Result<R, String>
where
    R: DeserializeOwned + CandidType,
{
    call(canister_id, method, ()).await
}

// ═══════════════════════════════════════════════════════════════
//  Logging Functions
// ═══════════════════════════════════════════════════════════════

fn log_call_start(canister_id: Principal, method: &str) {
    log_message(&format!("→ Calling {}.{}", canister_id, method));
}

fn log_call_start_with_cycles(canister_id: Principal, method: &str, cycles: u128) {
    log_message(&format!(
        "→ Calling {}.{} with {} cycles",
        canister_id, method, cycles
    ));
}

fn log_call_success(canister_id: Principal, method: &str) {
    log_message(&format!("✓ Call {}.{} succeeded", canister_id, method));
}

#[allow(deprecated)]
fn log_call_error(
    canister_id: Principal,
    method: &str,
    error: &(ic_cdk::api::call::RejectionCode, String),
) {
    log_message(&format!(
        "✗ Call {}.{} failed: {:?} - {}",
        canister_id, method, error.0, error.1
    ));
}

#[allow(deprecated)]
fn format_call_error(
    canister_id: Principal,
    method: &str,
    error: (ic_cdk::api::call::RejectionCode, String),
) -> String {
    format!(
        "Intercanister call to {}.{} failed: {:?} - {}",
        canister_id, method, error.0, error.1
    )
}

// ═══════════════════════════════════════════════════════════════
//  Logging Backend
// ═══════════════════════════════════════════════════════════════

fn log_message(msg: &str) {
    ic_cdk::println!("{}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_error_formatting() {
        let canister_id = Principal::anonymous();
        let error = (
            ic_cdk::api::call::RejectionCode::CanisterError,
            "Test error".to_string(),
        );

        let formatted = format_call_error(canister_id, "test_method", error);

        assert!(formatted.contains("test_method"));
        assert!(formatted.contains("Test error"));
    }
}