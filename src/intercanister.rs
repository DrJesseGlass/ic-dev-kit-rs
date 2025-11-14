// Intercanister call wrapper for ic-cdk 0.19
//
// Note: ic-cdk 0.19 is in a transitional state where the types have moved
// to ic_cdk::call but the functions are still in ic_cdk::api::call (deprecated).
// The replacement API mentioned in warnings doesn't actually exist yet.
// Using #[allow(deprecated)] is the correct approach until the API is fully updated.

use candid::{CandidType, Principal};
use serde::de::DeserializeOwned;

// ═══════════════════════════════════════════════════════════════
//  Core Call Functions
// ═══════════════════════════════════════════════════════════════

/// Make an intercanister call with automatic logging
#[allow(deprecated)]
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

    let result: Result<(R,), _> = ic_cdk::api::call::call(canister_id, method, (args,)).await;

    match &result {
        Ok(_) => log_call_success(canister_id, method),
        Err(e) => log_call_error(canister_id, method, e),
    }

    result
        .map(|r| r.0)
        .map_err(|e| format_call_error(canister_id, method, e))
}

/// Make an intercanister call with payment (cycles)
#[allow(deprecated)]
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

/// Make an intercanister call without waiting for response
#[allow(deprecated)]
pub fn call_one_way<T>(
    canister_id: Principal,
    method: &str,
    args: T,
) -> Result<(), String>
where
    T: CandidType,
{
    log_call_start(canister_id, method);

    let result: Result<(), _> = ic_cdk::api::call::notify(canister_id, method, (args,));

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

#[allow(deprecated)]
fn log_call_error(canister_id: Principal, method: &str, error: &(ic_cdk::api::call::RejectionCode, String)) {
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

/// Convenience function to call a method that takes no arguments
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
    #[allow(deprecated)]
    fn test_error_formatting() {
        let canister_id = Principal::anonymous();
        let error = (ic_cdk::api::call::RejectionCode::CanisterError, "Test error".to_string());

        let formatted = format_call_error(canister_id, "test_method", error);

        assert!(formatted.contains("test_method"));
        assert!(formatted.contains("Test error"));
    }
}