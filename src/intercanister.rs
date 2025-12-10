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

use candid::{CandidType, Principal};
use ic_cdk::call::{Call, CallFailed};
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
pub async fn call<T, R>(canister_id: Principal, method: &str, args: T) -> Result<R, String>
where
    T: candid::utils::ArgumentEncoder,
    R: DeserializeOwned + CandidType,
{
    log_call_start(canister_id, method);

    let result = Call::unbounded_wait(canister_id, method)
        .with_args(&args)
        .await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map_err(|e| format_call_error(canister_id, method, &e))
        .and_then(|response| {
            response
                .candid::<(R,)>()
                .map(|(r,)| r)
                .map_err(|e| format!("Failed to decode response: {}", e))
        })
}

/// Make a composite query call (query calling another query).
///
/// The caller must be marked with `#[query(composite = true)]`.
/// The target method must be a query method.
pub async fn query_call<T, R>(canister_id: Principal, method: &str, args: T) -> Result<R, String>
where
    T: candid::utils::ArgumentEncoder,
    R: DeserializeOwned + CandidType,
{
    log_call_start(canister_id, method);

    let result = Call::bounded_wait(canister_id, method)
        .with_args(&args)
        .await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map_err(|e| format_call_error(canister_id, method, &e))
        .and_then(|response| {
            response
                .candid::<(R,)>()
                .map(|(r,)| r)
                .map_err(|e| format!("Failed to decode response: {}", e))
        })
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

    let result = Call::unbounded_wait(canister_id, method)
        .with_args(&args)
        .with_cycles(cycles)
        .await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map_err(|e| format_call_error(canister_id, method, &e))
        .and_then(|response| {
            response
                .candid::<(R,)>()
                .map(|(r,)| r)
                .map_err(|e| format!("Failed to decode response: {}", e))
        })
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
pub fn call_one_way<T>(canister_id: Principal, method: &str, args: T) -> Result<(), String>
where
    T: candid::utils::ArgumentEncoder,
{
    log_call_start(canister_id, method);

    let result = Call::unbounded_wait(canister_id, method)
        .with_args(&args)
        .oneway();

    match &result {
        Ok(_) => {
            log_call_success(canister_id, method);
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Oneway call to {}.{} failed: {}", canister_id, method, e);
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

fn log_call_error(canister_id: Principal, method: &str, error: &CallFailed) {
    log_message(&format!(
        "✗ Call {}.{} failed: {}",
        canister_id, method, error
    ));
}

fn format_call_error(canister_id: Principal, method: &str, error: &CallFailed) -> String {
    format!(
        "Intercanister call to {}.{} failed: {}",
        canister_id, method, error
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
    use ic_cdk::call::CallRejected;

    #[test]
    fn test_error_formatting() {
        let canister_id = Principal::anonymous();
        let error = CallFailed::CallRejected(CallRejected::with_rejection(
            5, // CanisterError
            "Test error".to_string(),
        ));

        let formatted = format_call_error(canister_id, "test_method", &error);

        assert!(formatted.contains("test_method"));
        assert!(formatted.contains("Test error"));
    }
}
