//! Authentication module for Internet Computer canisters.
//!
//! Provides principal-based authorization with guard functions for IC CDK.
//!
//! # Quick Start
//!
//! ```rust,ignore
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
//! ```rust,ignore
//! #[ic_cdk::pre_upgrade]
//! fn pre_upgrade() {
//!     let bytes = auth::save_to_bytes();
//!     // Store bytes in stable memory
//! }
//!
//! #[ic_cdk::post_upgrade]
//! fn post_upgrade() {
//!     // Load bytes from stable memory
//!     auth::init_from_saved(Some(bytes));
//! }
//! ```

use candid::Principal;
use ic_cdk;
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
    /// An error occurred while accessing storage.
    #[error("Storage error: {0}")]
    StorageError(String),
    /// An error occurred during serialization/deserialization.
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Result type for authentication operations.
pub type AuthResult<T> = Result<T, AuthError>;

// ═══════════════════════════════════════════════════════════════
//  Storage Implementation
// ═══════════════════════════════════════════════════════════════

/// Simple in-memory storage for authorized principals.
///
/// This is used internally by [`Auth`] to persist the set of authorized principals.
pub struct AuthStorage {
    principals: RefCell<HashSet<Principal>>,
}

impl AuthStorage {
    /// Create a new empty storage.
    pub fn new() -> Self {
        Self {
            principals: RefCell::new(HashSet::new()),
        }
    }

    /// Create storage with an initial authorized principal.
    pub fn with_initial_principal(principal: Principal) -> Self {
        let mut principals = HashSet::new();
        principals.insert(principal);
        Self {
            principals: RefCell::new(principals),
        }
    }

    /// Save principals to storage.
    pub fn save_principals(&self, principals: &HashSet<Principal>) -> AuthResult<()> {
        *self.principals.borrow_mut() = principals.clone();
        Ok(())
    }

    /// Load principals from storage.
    pub fn load_principals(&self) -> AuthResult<HashSet<Principal>> {
        Ok(self.principals.borrow().clone())
    }
}

impl Default for AuthStorage {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Auth Manager
// ═══════════════════════════════════════════════════════════════

/// Main authentication manager for IC canisters.
///
/// Manages a set of authorized principals with caching for fast lookups.
///
/// # Example
///
/// ```rust,ignore
/// let storage = AuthStorage::new();
/// let auth = Auth::new(storage);
/// auth.add_principal(my_principal)?;
/// assert!(auth.is_authorized(&my_principal)?);
/// ```
pub struct Auth {
    storage: AuthStorage,
    cache: RefCell<HashSet<Principal>>,
}

impl Auth {
    /// Create a new Auth manager with the given storage backend.
    pub fn new(storage: AuthStorage) -> Self {
        let auth = Self {
            storage,
            cache: RefCell::new(HashSet::new()),
        };

        // Load from storage into cache
        if let Ok(principals) = auth.storage.load_principals() {
            *auth.cache.borrow_mut() = principals;
        }

        auth
    }

    /// Check if a principal is authorized.
    pub fn is_authorized(&self, principal: &Principal) -> AuthResult<bool> {
        Ok(self.cache.borrow().contains(principal))
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
    ///
    /// Combines [`get_current_principal`](Self::get_current_principal) and
    /// [`is_authorized`](Self::is_authorized).
    pub fn check_authorized(&self) -> AuthResult<()> {
        let current = self.get_current_principal()?;
        if self.is_authorized(&current)? {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }

    /// Add a principal to the authorized set.
    pub fn add_principal(&self, principal: Principal) -> AuthResult<()> {
        self.cache.borrow_mut().insert(principal);
        Ok(())
    }

    /// Remove a principal from the authorized set.
    pub fn remove_principal(&self, principal: &Principal) -> AuthResult<()> {
        self.cache.borrow_mut().remove(principal);
        Ok(())
    }

    /// List all authorized principals.
    pub fn list_principals(&self) -> AuthResult<Vec<Principal>> {
        Ok(self.cache.borrow().iter().cloned().collect())
    }

    /// Ensure a principal is authorized (add if not present).
    pub fn ensure_authorized(&self, principal: Principal) -> AuthResult<()> {
        self.add_principal(principal)
    }

    /// Save current cache to storage.
    pub fn save_to_storage(&self) -> AuthResult<()> {
        let cache = self.cache.borrow();
        self.storage.save_principals(&cache)
    }

    /// Load from storage to cache.
    pub fn load_from_storage(&self) -> AuthResult<()> {
        let principals = self.storage.load_principals()?;
        *self.cache.borrow_mut() = principals;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════
//  Global Auth Instance (Thread-Local for IC)
// ═══════════════════════════════════════════════════════════════

thread_local! {
    static AUTH: RefCell<Option<Auth>> = RefCell::new(None);
}

/// Initialize the auth system with empty storage.
///
/// Call this in your `#[ic_cdk::init]` function if you want to start
/// with no authorized principals.
pub fn init() {
    let storage = AuthStorage::new();
    let auth = Auth::new(storage);
    AUTH.with(|a| *a.borrow_mut() = Some(auth));
}

/// Initialize auth system with the deployer as initial authorized principal.
///
/// This is the most common initialization pattern. The caller of the
/// `init` function (typically the deployer) becomes the first authorized principal.
///
/// # Example
///
/// ```rust,ignore
/// #[ic_cdk::init]
/// fn init() {
///     auth::init_with_caller();
/// }
/// ```
pub fn init_with_caller() {
    let caller = ic_cdk::api::msg_caller();
    let storage = AuthStorage::with_initial_principal(caller);
    let auth = Auth::new(storage);
    AUTH.with(|a| *a.borrow_mut() = Some(auth));
}

/// Initialize the auth system with specific principals.
///
/// Useful when you need to pre-authorize multiple principals at init time.
pub fn init_with_principals(principals: Vec<Principal>) {
    let mut initial_set = HashSet::new();
    for principal in principals {
        initial_set.insert(principal);
    }

    let storage = AuthStorage {
        principals: RefCell::new(initial_set),
    };
    let auth = Auth::new(storage);
    AUTH.with(|a| *a.borrow_mut() = Some(auth));
}

/// Initialize auth system from saved bytes (for post-upgrade).
///
/// If deserialization fails, falls back to initializing with the caller.
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

/// Helper function to work with the auth instance.
fn with_auth<R, F>(f: F) -> R
where
    F: FnOnce(&Auth) -> R,
{
    AUTH.with(|a| {
        let auth_ref = a.borrow();
        let auth = auth_ref
            .as_ref()
            .expect("Auth not initialized - call auth::init() first");
        f(auth)
    })
}

// ═══════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════

/// Guard function for IC CDK queries/updates.
///
/// Use this as a guard in `#[ic_cdk::update]` or `#[ic_cdk::query]` attributes.
///
/// # Example
///
/// ```rust,ignore
/// #[ic_cdk::update(guard = "auth::is_authorized")]
/// fn admin_function() {
///     // Only authorized principals reach here
/// }
/// ```
pub fn is_authorized() -> Result<(), String> {
    with_auth(|auth| {
        auth.check_authorized()
            .map_err(|e| format!("Authorization failed: {}", e))
    })
}

/// Alias for [`is_authorized`] - check if current caller is authorized.
pub fn check() -> Result<(), String> {
    is_authorized()
}

/// Add a principal to the authorized set.
///
/// # Errors
///
/// Returns an error string if the operation fails.
pub fn add_principal(principal: Principal) -> Result<(), String> {
    with_auth(|auth| {
        auth.add_principal(principal)
            .map_err(|e| format!("Failed to add principal: {}", e))
    })
}

/// Remove a principal from the authorized set.
///
/// # Returns
///
/// Success message or error string.
pub fn remove_principal(principal: Principal) -> Result<String, String> {
    with_auth(|auth| {
        auth.remove_principal(&principal)
            .map_err(|e| format!("Failed to remove principal: {}", e))?;
        Ok("Successfully removed principal from allowlist".to_string())
    })
}

/// Check if a specific principal is authorized.
pub fn is_principal_authorized(principal: Principal) -> Result<bool, String> {
    with_auth(|auth| {
        auth.is_authorized(&principal)
            .map_err(|e| format!("Failed to check authorization: {}", e))
    })
}

/// List all authorized principals.
pub fn list_principals() -> Result<Vec<Principal>, String> {
    with_auth(|auth| {
        auth.list_principals()
            .map_err(|e| format!("Failed to list principals: {}", e))
    })
}

/// Ensure a principal is authorized (add if not present).
pub fn ensure_authorized(principal: Principal) -> Result<(), String> {
    with_auth(|auth| {
        auth.ensure_authorized(principal)
            .map_err(|e| format!("Failed to ensure authorization: {}", e))
    })
}

// ═══════════════════════════════════════════════════════════════
//  Serialization Utilities (for upgrade persistence)
// ═══════════════════════════════════════════════════════════════

/// Save auth principals to bytes for stable storage.
///
/// Call this in `#[ic_cdk::pre_upgrade]` and store the result.
pub fn save_to_bytes() -> Vec<u8> {
    with_auth(|auth| {
        let principals = auth.list_principals().unwrap_or_default();
        candid::encode_args((&principals,)).unwrap_or_default()
    })
}

/// Load auth principals from bytes (for post-upgrade).
///
/// # Errors
///
/// Returns an error if deserialization fails.
pub fn load_from_bytes(bytes: &[u8]) -> Result<(), String> {
    let decoded: Result<(Vec<Principal>,), _> = candid::decode_args(bytes);
    match decoded {
        Ok((principals,)) => {
            with_auth(|auth| {
                auth.cache.borrow_mut().clear();
                for principal in principals {
                    let _ = auth.add_principal(principal);
                }
            });
            Ok(())
        }
        Err(e) => Err(format!("Failed to decode principals: {:?}", e)),
    }
}

/// Validate a principal text string.
///
/// # Example
///
/// ```rust,ignore
/// let principal = auth::validate_principal_text("2vxsx-fae")?;
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
/// - `deauthorize_principal` - Remove a principal (guarded)
/// - `get_authorized_principals` - List all principals (guarded)
/// - `check_principal_authorized` - Check if a principal is authorized (guarded)
/// - `get_authorized_count` - Get count of authorized principals (guarded)
///
/// # Example
///
/// ```rust,ignore
/// ic_dev_kit_rs::export_auth_endpoints!();
/// ```
#[macro_export]
macro_rules! export_auth_endpoints {
    () => {
        fn is_authorized() -> Result<(), String> {
            $crate::auth::is_authorized()
        }

        #[ic_cdk::update(guard = "is_authorized")]
        fn authorize_principal(principal: candid::Principal) {
            let _ = $crate::auth::add_principal(principal);
        }

        #[ic_cdk::update(guard = "is_authorized")]
        fn deauthorize_principal(principal: candid::Principal) -> String {
            $crate::auth::remove_principal(principal).unwrap_or_else(|e| e)
        }

        #[ic_cdk::query(guard = "is_authorized")]
        fn get_authorized_principals() -> Vec<candid::Principal> {
            $crate::auth::list_principals().unwrap_or_default()
        }

        #[ic_cdk::query(guard = "is_authorized")]
        fn check_principal_authorized(principal: candid::Principal) -> bool {
            $crate::auth::is_principal_authorized(principal).unwrap_or(false)
        }

        #[ic_cdk::query(guard = "is_authorized")]
        fn get_authorized_count() -> usize {
            $crate::auth::list_principals().map(|list| list.len()).unwrap_or(0)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_storage() {
        let storage = AuthStorage::new();
        let mut principals = HashSet::new();
        principals.insert(Principal::anonymous());

        storage.save_principals(&principals).unwrap();
        let loaded = storage.load_principals().unwrap();

        assert_eq!(principals, loaded);
    }

    #[test]
    fn test_auth_manager() {
        let storage = AuthStorage::new();
        let auth = Auth::new(storage);

        let test_principal = Principal::anonymous();

        // Test adding principal
        auth.add_principal(test_principal).unwrap();
        assert!(auth.is_authorized(&test_principal).unwrap());

        // Test listing
        let list = auth.list_principals().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list.contains(&test_principal));

        // Test removing principal
        auth.remove_principal(&test_principal).unwrap();
        assert!(!auth.is_authorized(&test_principal).unwrap());
    }

    #[test]
    fn test_principal_validation() {
        let result = validate_principal_text("2vxsx-fae");
        assert!(result.is_ok());

        let result = validate_principal_text("invalid");
        assert!(result.is_err());
    }
}