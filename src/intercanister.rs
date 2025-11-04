// Intercanister call wrapper with timeout and logging
//
// This module provides a single abstraction point for all intercanister calls,
// making it easy to update call patterns across your entire codebase.
//
// ## Benefits
//
// - **DRY**: Update timeout logic in one place
// - **Logging**: Automatic logging of all intercanister calls
// - **Error handling**: Consistent error formatting
// - **Future-proof**: Easy to add retries, metrics, etc.
//
// ## Usage
//
// ```rust
// use ic_dev_kit_rs::intercanister;
// use candid::Principal;
//
// #[derive(CandidType)]
// struct MyRequest {
//     data: String,
// }
//
// #[derive(CandidType, Deserialize)]
// struct MyResponse {
//     result: u64,
// }
//
// async fn call_other_canister() -> Result<MyResponse, String> {
//     let canister_id = Principal::from_text("...").unwrap();
//
//     let request = MyRequest {
//         data: "hello".to_string(),
//     };
//
//     // Simple call
//     intercanister::call(canister_id, "my_method", request).await
// }
// ```

use candid::{CandidType, Principal};
use ic_cdk;
use serde::de::DeserializeOwned;

// ═══════════════════════════════════════════════════════════════
//  Core Call Functions
// ═══════════════════════════════════════════════════════════════

/// Make an intercanister call with automatic logging
///
/// This is the primary function for intercanister calls. It:
/// - Logs the call before and after execution
/// - Uses proper error handling
/// - Returns a Result for easy error propagation
///
/// ## Example
/// ```rust,ignore
/// let response: MyResponse = intercanister::call(
///     canister_id,
///     "my_method",
///     MyRequest { data: "test".to_string() }
/// ).await?;
/// ```
pub async fn call<T, R>(
    canister_id: Principal,
    method: &str,
    args: T,
) -> Result<R, String>
where
    T: CandidType,
    R: DeserializeOwned + CandidType,
{
    log_call_start(canister_id, method);

    let result: Result<(R,), _> = ic_cdk::call(canister_id, method, (args,)).await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map(|r| r.0)
        .map_err(|e| format_call_error(canister_id, method, e))
}

/// Make an intercanister call with payment (cycles)
///
/// Use this when you need to send cycles with the call.
pub async fn call_with_payment<T, R>(
    canister_id: Principal,
    method: &str,
    args: T,
    cycles: u128,
) -> Result<R, String>
where
    T: CandidType,
    R: DeserializeOwned + CandidType,
{
    log_call_start_with_cycles(canister_id, method, cycles);

    let result: Result<(R,), _> =
        ic_cdk::api::call::call_with_payment128(canister_id, method, (args,), cycles).await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map(|r| r.0)
        .map_err(|e| format_call_error(canister_id, method, e))
}

/// Make an intercanister call without waiting for response (fire and forget)
///
/// Note: This still returns a Result, but you don't get the response data.
/// Useful for notifications or when you don't care about the result.
pub async fn call_one_way<T>(
    canister_id: Principal,
    method: &str,
    args: T,
) -> Result<(), String>
where
    T: CandidType,
{
    log_call_start(canister_id, method);

    let result: Result<(), _> = ic_cdk::notify(canister_id, method, (args,));

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

fn log_call_error(canister_id: Principal, method: &str, error: &(ic_cdk::api::call::RejectionCode, String)) {
    log_message(&format!(
        "✗ Call {}.{} failed: {:?} - {}",
        canister_id, method, error.0, error.1
    ));
}

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
//  Logging Backend (can be customized)
// ═══════════════════════════════════════════════════════════════

/// Log a message
///
/// By default, this uses ic_cdk::println!. If you want to integrate with
/// the telemetry module or another logging system, modify this function.
fn log_message(msg: &str) {
    // Option 1: Simple println (default)
    ic_cdk::println!("{}", msg);

    // Option 2: Integrate with telemetry module (uncomment if using telemetry)
    // crate::telemetry::log_info(msg);
}

/// Convenience function to call the method that takes no arguments
pub async fn call_no_args<R>(
    canister_id: Principal,
    method: &str,
) -> Result<R, String>
where
    R: DeserializeOwned + CandidType,
{
    #[derive(CandidType)]
    struct NoArgs {}

    call(canister_id, method, NoArgs {}).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_formatting() {
        let canister_id = Principal::anonymous();
        let error = (ic_cdk::api::call::RejectionCode::CanisterError, "Test error".to_string());

        let formatted = format_call_error(canister_id, "test_method", error);

        assert!(formatted.contains("test_method"));
        assert!(formatted.contains("Test error"));
    }
}