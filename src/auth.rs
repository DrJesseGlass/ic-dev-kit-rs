//! Authentication module for Internet Computer canisters.
//!
//! Provides principal-based authorization with guard functions for IC CDK.
//!
//! The authorized set lives on the Wasm heap. Persist it across upgrades with
//! [`save_to_bytes`] / [`init_from_saved`] as shown below.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use ic_dev_kit_rs::auth;
//!
//! #[ic_cdk::init]
//! fn init() {
//!     auth::init_with_caller(); // Deployer becomes first authorized principal
//! }
//!
//! #[ic_cdk::update(guard = "auth::is_authorized")]
//! fn admin_only() {
//!     // Only authorized principals can call this
//! }
//! ```
//!
//! # Upgrade Persistence
//!
//! ```rust,no_run
//! # use ic_dev_kit_rs::auth;
//! #[ic_cdk::pre_upgrade]
//! fn pre_upgrade() {
//!     let bytes = auth::save_to_bytes();
//!     // Write `bytes` to stable memory (see the README's Upgrade Persistence).
//! #   let _ = bytes;
//! }
//!
//! #[ic_cdk::post_upgrade]
//! fn post_upgrade() {
//!     // Read the bytes back from stable memory; `None` if there were none.
//! #   let bytes: Option<Vec<u8>> = None;
//!     auth::init_from_saved(bytes);
//! }
//! ```
//!
//! **Recovery behavior:** if [`init_from_saved`] receives `None` or bytes that
//! fail to decode, it falls back to authorizing the caller of the upgrade
//! (i.e. whoever ran `dfx deploy`). This prevents locking yourself out of a
//! canister with corrupt auth data, at the cost of silently replacing the
//! allowlist in that failure case — the fallback is logged via
//! `ic_cdk::println!`.

use candid::Principal;
use std::cell::RefCell;
use std::collections::HashSet;

// ═══════════════════════════════════════════════════════════════
//  Error Types
// ═══════════════════════════════════════════════════════════════

/// Errors that can occur during authentication operations.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The caller is not authorized to perform this action.
    #[error("Unauthorized")]
    Unauthorized,
    /// The provided principal text is invalid.
    #[error("Invalid principal")]
    InvalidPrincipal,
    /// Removing this principal would leave the authorized set empty.
    #[error("Cannot remove the last authorized principal")]
    LastPrincipal,
}

/// Result type for authentication operations.
pub type AuthResult<T> = Result<T, AuthError>;

// ═══════════════════════════════════════════════════════════════
//  Auth Manager
// ═══════════════════════════════════════════════════════════════

/// Main authentication manager for IC canisters.
///
/// Holds the set of authorized principals. The set lives on the heap; use
/// [`save_to_bytes`] / [`init_from_saved`] to persist it across upgrades.
///
/// # Example
///
/// ```rust
/// # use ic_dev_kit_rs::auth::Auth;
/// # let my_principal = candid::Principal::anonymous();
/// let auth = Auth::new();
/// auth.add_principal(my_principal);
/// assert!(auth.is_authorized(&my_principal));
/// ```
pub struct Auth {
    principals: RefCell<HashSet<Principal>>,
}

impl Auth {
    /// Create a new Auth manager with no authorized principals.
    pub fn new() -> Self {
        Self {
            principals: RefCell::new(HashSet::new()),
        }
    }

    /// Create an Auth manager pre-populated with the given principals.
    pub fn with_principals(principals: impl IntoIterator<Item = Principal>) -> Self {
        Self {
            principals: RefCell::new(principals.into_iter().collect()),
        }
    }

    /// Check if a principal is authorized.
    pub fn is_authorized(&self, principal: &Principal) -> bool {
        self.principals.borrow().contains(principal)
    }

    /// Get the current caller principal.
    ///
    /// Returns an error if the caller is the canister itself.
    pub fn get_current_principal(&self) -> AuthResult<Principal> {
        let caller = ic_cdk::api::msg_caller();
        if caller == ic_cdk::api::canister_self() {
            return Err(AuthError::Unauthorized);
        }
        Ok(caller)
    }

    /// Check if the current caller is authorized.
    pub fn check_authorized(&self) -> AuthResult<()> {
        let current = self.get_current_principal()?;
        if self.is_authorized(&current) {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }

    /// Add a principal to the authorized set.
    pub fn add_principal(&self, principal: Principal) {
        self.principals.borrow_mut().insert(principal);
    }

    /// Remove a principal from the authorized set.
    ///
    /// Returns `true` if the principal was present. This is a plain set
    /// operation; the "never remove the last admin" policy lives in the
    /// module-level [`remove_principal`].
    pub fn remove_principal(&self, principal: &Principal) -> bool {
        self.principals.borrow_mut().remove(principal)
    }

    /// Number of authorized principals.
    pub fn len(&self) -> usize {
        self.principals.borrow().len()
    }

    /// Whether the authorized set is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// List all authorized principals.
    pub fn list_principals(&self) -> Vec<Principal> {
        self.principals.borrow().iter().cloned().collect()
    }

    /// Replace the entire authorized set.
    pub fn set_principals(&self, principals: impl IntoIterator<Item = Principal>) {
        *self.principals.borrow_mut() = principals.into_iter().collect();
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Global Auth Instance (Thread-Local for IC)
// ═══════════════════════════════════════════════════════════════

thread_local! {
    static AUTH: RefCell<Option<Auth>> = const { RefCell::new(None) };
}

/// Initialize the auth system with no authorized principals.
///
/// Call this in your `#[ic_cdk::init]` function if you want to start
/// with no authorized principals.
pub fn init() {
    AUTH.with(|a| *a.borrow_mut() = Some(Auth::new()));
}

/// Initialize auth system with the deployer as initial authorized principal.
///
/// This is the most common initialization pattern. The caller of the
/// `init` function (typically the deployer) becomes the first authorized principal.
///
/// # Example
///
/// ```rust,no_run
/// # use ic_dev_kit_rs::auth;
/// #[ic_cdk::init]
/// fn init() {
///     auth::init_with_caller();
/// }
/// ```
pub fn init_with_caller() {
    let caller = ic_cdk::api::msg_caller();
    AUTH.with(|a| *a.borrow_mut() = Some(Auth::with_principals([caller])));
}

/// Initialize the auth system with specific principals.
///
/// Useful when you need to pre-authorize multiple principals at init time.
pub fn init_with_principals(principals: Vec<Principal>) {
    AUTH.with(|a| *a.borrow_mut() = Some(Auth::with_principals(principals)));
}

/// Initialize auth system from saved bytes (for post-upgrade).
///
/// If `saved_bytes` is `None` or fails to decode, falls back to authorizing
/// the caller (the principal performing the upgrade) so the canister is never
/// left without an admin. The fallback is logged.
pub fn init_from_saved(saved_bytes: Option<Vec<u8>>) {
    let principals = if let Some(bytes) = saved_bytes {
        match candid::decode_args::<(Vec<Principal>,)>(&bytes) {
            Ok((principals,)) => {
                ic_cdk::println!("Restored {} principals from saved data", principals.len());
                principals
            }
            Err(e) => {
                ic_cdk::println!("Failed to decode saved principals: {:?}, starting fresh", e);
                vec![ic_cdk::api::msg_caller()]
            }
        }
    } else {
        ic_cdk::println!("No saved principals found, starting fresh");
        vec![ic_cdk::api::msg_caller()]
    };

    init_with_principals(principals);
}

/// Error returned by the public auth functions when [`init`] was never called.
const NOT_INITIALIZED_ERR: &str = "Auth not initialized - call auth::init() first";

/// Helper to work with the auth instance without panicking when
/// uninitialized. Guards built on this reject calls instead of trapping.
fn with_auth<R, F>(f: F) -> Result<R, String>
where
    F: FnOnce(&Auth) -> R,
{
    AUTH.with(|a| {
        a.borrow()
            .as_ref()
            .map(f)
            .ok_or_else(|| NOT_INITIALIZED_ERR.to_string())
    })
}

// ═══════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════

/// Guard function for IC CDK queries/updates.
///
/// Use this as a guard in `#[ic_cdk::update]` or `#[ic_cdk::query]` attributes.
/// Rejects (rather than traps) if the auth system was never initialized.
///
/// # Example
///
/// ```rust,no_run
/// # use ic_dev_kit_rs::auth;
/// #[ic_cdk::update(guard = "auth::is_authorized")]
/// fn admin_function() {
///     // Only authorized principals reach here
/// }
/// ```
pub fn is_authorized() -> Result<(), String> {
    with_auth(|auth| auth.check_authorized())?.map_err(|e| format!("Authorization failed: {}", e))
}

/// Add a principal to the authorized set.
///
/// # Errors
///
/// Returns an error string if auth is not initialized.
pub fn add_principal(principal: Principal) -> Result<(), String> {
    with_auth(|auth| auth.add_principal(principal))
}

/// Remove a principal from the authorized set.
///
/// Refuses to remove the last authorized principal: emptying the set would
/// lock every guarded endpoint until the next upgrade. Add a replacement
/// first. Removing a principal that is not in the set succeeds, with a
/// message saying it was absent.
///
/// # Returns
///
/// A message describing what happened, or an error string if auth is not
/// initialized or `principal` is the only authorized principal.
pub fn remove_principal(principal: Principal) -> Result<String, String> {
    with_auth(|auth| {
        if auth.len() == 1 && auth.is_authorized(&principal) {
            return Err(AuthError::LastPrincipal.to_string());
        }
        Ok(if auth.remove_principal(&principal) {
            "Successfully removed principal from allowlist".to_string()
        } else {
            "Principal was not in the allowlist".to_string()
        })
    })?
}

/// Check if a specific principal is authorized.
pub fn is_principal_authorized(principal: Principal) -> Result<bool, String> {
    with_auth(|auth| auth.is_authorized(&principal))
}

/// List all authorized principals.
pub fn list_principals() -> Result<Vec<Principal>, String> {
    with_auth(|auth| auth.list_principals())
}

// ═══════════════════════════════════════════════════════════════
//  Serialization Utilities (for upgrade persistence)
// ═══════════════════════════════════════════════════════════════

/// Save auth principals to bytes for stable storage.
///
/// Call this in `#[ic_cdk::pre_upgrade]` and store the result. Returns an
/// empty vec if auth was never initialized.
pub fn save_to_bytes() -> Vec<u8> {
    let principals = list_principals().unwrap_or_default();
    candid::encode_args((&principals,)).unwrap_or_default()
}

/// Load auth principals from bytes, replacing the current set.
///
/// # Errors
///
/// Returns an error if deserialization fails or auth is not initialized.
pub fn load_from_bytes(bytes: &[u8]) -> Result<(), String> {
    let (principals,): (Vec<Principal>,) =
        candid::decode_args(bytes).map_err(|e| format!("Failed to decode principals: {:?}", e))?;
    with_auth(|auth| auth.set_principals(principals))
}

/// Validate a principal text string.
///
/// # Example
///
/// ```rust
/// # use ic_dev_kit_rs::auth;
/// # fn main() -> Result<(), auth::AuthError> {
/// let principal = auth::validate_principal_text("2vxsx-fae")?;
/// # assert_eq!(principal, candid::Principal::anonymous());
/// # Ok(())
/// # }
/// ```
pub fn validate_principal_text(text: &str) -> Result<Principal, AuthError> {
    Principal::from_text(text).map_err(|_| AuthError::InvalidPrincipal)
}

// ═══════════════════════════════════════════════════════════════
//  Macro for Exporting Auth Endpoints
// ═══════════════════════════════════════════════════════════════

/// Generate standard auth management endpoints.
///
/// This macro creates the following IC endpoints:
/// - `authorize_principal` - Add a principal (guarded)
/// - `deauthorize_principal` - Remove a principal (guarded; refuses to remove
///   the last one)
/// - `get_authorized_principals` - List all principals (guarded)
/// - `check_principal_authorized` - Check if a principal is authorized (guarded)
/// - `get_authorized_count` - Get count of authorized principals (guarded)
///
/// It also defines a local `is_authorized` guard function delegating to
/// [`auth::is_authorized`](crate::auth::is_authorized), which your own
/// endpoints may reference with `guard = "is_authorized"`.
///
/// # Example
///
/// ```rust,no_run
/// ic_dev_kit_rs::export_auth_endpoints!();
/// ```
#[macro_export]
macro_rules! export_auth_endpoints {
    () => {
        fn is_authorized() -> Result<(), String> {
            $crate::auth::is_authorized()
        }

        #[ic_cdk::update(guard = "is_authorized")]
        fn authorize_principal(principal: ::candid::Principal) -> Result<(), String> {
            $crate::auth::add_principal(principal)
        }

        #[ic_cdk::update(guard = "is_authorized")]
        fn deauthorize_principal(principal: ::candid::Principal) -> Result<String, String> {
            $crate::auth::remove_principal(principal)
        }

        #[ic_cdk::query(guard = "is_authorized")]
        fn get_authorized_principals() -> Vec<::candid::Principal> {
            $crate::auth::list_principals().unwrap_or_default()
        }

        #[ic_cdk::query(guard = "is_authorized")]
        fn check_principal_authorized(principal: ::candid::Principal) -> bool {
            $crate::auth::is_principal_authorized(principal).unwrap_or(false)
        }

        #[ic_cdk::query(guard = "is_authorized")]
        fn get_authorized_count() -> usize {
            $crate::auth::list_principals()
                .map(|list| list.len())
                .unwrap_or(0)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_manager() {
        let auth = Auth::new();

        let test_principal = Principal::anonymous();

        // Test adding principal
        auth.add_principal(test_principal);
        assert!(auth.is_authorized(&test_principal));

        // Test listing
        let list = auth.list_principals();
        assert_eq!(list.len(), 1);
        assert!(list.contains(&test_principal));

        // Test removing principal
        assert!(auth.remove_principal(&test_principal));
        assert!(!auth.is_authorized(&test_principal));
        assert!(!auth.remove_principal(&test_principal));
        assert!(auth.is_empty());
    }

    #[test]
    fn test_cannot_remove_last_principal() {
        let only = Principal::anonymous();
        let other = Principal::from_slice(&[9]);
        init_with_principals(vec![only]);

        // Removing the only principal is refused and leaves it in place.
        let err = remove_principal(only).unwrap_err();
        assert!(err.contains("last authorized principal"), "{err}");
        assert_eq!(list_principals().unwrap(), vec![only]);

        // Removing an absent principal succeeds but says so, even with one left.
        let msg = remove_principal(other).unwrap();
        assert!(msg.contains("not in the allowlist"), "{msg}");
        assert_eq!(list_principals().unwrap(), vec![only]);

        // With a replacement in place, removal succeeds.
        add_principal(other).unwrap();
        remove_principal(only).unwrap();
        assert_eq!(list_principals().unwrap(), vec![other]);
    }

    #[test]
    fn test_with_principals() {
        let p = Principal::anonymous();
        let auth = Auth::with_principals([p]);
        assert!(auth.is_authorized(&p));
        assert_eq!(auth.list_principals().len(), 1);
    }

    #[test]
    fn test_uninitialized_guard_rejects_instead_of_trapping() {
        // Fresh test thread => thread-local AUTH is None.
        assert!(is_authorized().is_err());
        assert!(add_principal(Principal::anonymous()).is_err());
        assert!(list_principals().is_err());
        // save_to_bytes must not trap; it encodes an empty list.
        let bytes = save_to_bytes();
        let (decoded,): (Vec<Principal>,) = candid::decode_args(&bytes).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_principal_validation() {
        let result = validate_principal_text("2vxsx-fae");
        assert!(result.is_ok());

        let result = validate_principal_text("invalid");
        assert!(result.is_err());
    }
}
