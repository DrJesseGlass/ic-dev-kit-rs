// Authentication module for Internet Computer canisters
use candid::Principal;
use ic_cdk;
use std::cell::RefCell;
use std::collections::HashSet;

// ═══════════════════════════════════════════════════════════════
//  Error Types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Invalid principal")]
    InvalidPrincipal,
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type AuthResult<T> = Result<T, AuthError>;

// ═══════════════════════════════════════════════════════════════
//  Storage Implementation
// ═══════════════════════════════════════════════════════════════

/// Simple in-memory storage for authorized principals
pub struct AuthStorage {
    principals: RefCell<HashSet<Principal>>,
}

impl AuthStorage {
    pub fn new() -> Self {
        Self {
            principals: RefCell::new(HashSet::new()),
        }
    }

    pub fn with_initial_principal(principal: Principal) -> Self {
        let mut principals = HashSet::new();
        principals.insert(principal);
        Self {
            principals: RefCell::new(principals),
        }
    }

    pub fn save_principals(&self, principals: &HashSet<Principal>) -> AuthResult<()> {
        *self.principals.borrow_mut() = principals.clone();
        Ok(())
    }

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

/// Main authentication manager for IC canisters
pub struct Auth {
    storage: AuthStorage,
    cache: RefCell<HashSet<Principal>>,
}

impl Auth {
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

    /// Check if a principal is authorized
    pub fn is_authorized(&self, principal: &Principal) -> AuthResult<bool> {
        Ok(self.cache.borrow().contains(principal))
    }

    /// Get the current caller principal
    pub fn get_current_principal(&self) -> AuthResult<Principal> {
        let caller = ic_cdk::api::caller();
        if caller == ic_cdk::api::id() {
            return Err(AuthError::Unauthorized);
        }
        Ok(caller)
    }

    /// Check if current caller is authorized
    pub fn check_authorized(&self) -> AuthResult<()> {
        let current = self.get_current_principal()?;
        if self.is_authorized(&current)? {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }

    /// Add an authorized principal
    pub fn add_principal(&self, principal: Principal) -> AuthResult<()> {
        self.cache.borrow_mut().insert(principal);
        Ok(())
    }

    /// Remove an authorized principal
    pub fn remove_principal(&self, principal: &Principal) -> AuthResult<()> {
        self.cache.borrow_mut().remove(principal);
        Ok(())
    }

    /// List all authorized principals
    pub fn list_principals(&self) -> AuthResult<Vec<Principal>> {
        Ok(self.cache.borrow().iter().cloned().collect())
    }

    /// Ensure a principal is authorized (add if not present)
    pub fn ensure_authorized(&self, principal: Principal) -> AuthResult<()> {
        self.add_principal(principal)
    }

    /// Save current cache to storage
    pub fn save_to_storage(&self) -> AuthResult<()> {
        let cache = self.cache.borrow();
        self.storage.save_principals(&cache)
    }

    /// Load from storage to cache
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

/// Initialize the auth system with simple in-memory storage
pub fn init() {
    let storage = AuthStorage::new();
    let auth = Auth::new(storage);
    AUTH.with(|a| *a.borrow_mut() = Some(auth));
}

/// Initialize auth system with the deployer as initial authorized principal
pub fn init_with_caller() {
    let caller = ic_cdk::api::caller();
    let storage = AuthStorage::with_initial_principal(caller);
    let auth = Auth::new(storage);
    AUTH.with(|a| *a.borrow_mut() = Some(auth));
}

/// Initialize the auth system with specific principals
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

/// Initialize auth system from saved bytes (for post-upgrade)
pub fn init_from_saved(saved_bytes: Option<Vec<u8>>) {
    let principals = if let Some(bytes) = saved_bytes {
        match candid::decode_args::<(Vec<Principal>,)>(&bytes) {
            Ok((principals,)) => {
                ic_cdk::println!("Restored {} principals from saved data", principals.len());
                principals
            }
            Err(e) => {
                ic_cdk::println!("Failed to decode saved principals: {:?}, starting fresh", e);
                vec![ic_cdk::api::caller()]
            }
        }
    } else {
        ic_cdk::println!("No saved principals found, starting fresh");
        vec![ic_cdk::api::caller()]
    };

    init_with_principals(principals);
}

/// Helper function to work with the auth instance
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

/// Guard function for IC CDK queries/updates
pub fn is_authorized() -> Result<(), String> {
    with_auth(|auth| {
        auth.check_authorized()
            .map_err(|e| format!("Authorization failed: {}", e))
    })
}

/// Check if current caller is authorized
pub fn check() -> Result<(), String> {
    is_authorized()
}

/// Add an authorized principal
pub fn add_principal(principal: Principal) -> Result<(), String> {
    with_auth(|auth| {
        auth.add_principal(principal)
            .map_err(|e| format!("Failed to add principal: {}", e))
    })
}

/// Remove an authorized principal
pub fn remove_principal(principal: Principal) -> Result<String, String> {
    with_auth(|auth| {
        auth.remove_principal(&principal)
            .map_err(|e| format!("Failed to remove principal: {}", e))?;
        Ok("Successfully removed principal from allowlist".to_string())
    })
}

/// Check if a specific principal is authorized
pub fn is_principal_authorized(principal: Principal) -> Result<bool, String> {
    with_auth(|auth| {
        auth.is_authorized(&principal)
            .map_err(|e| format!("Failed to check authorization: {}", e))
    })
}

/// List all authorized principals
pub fn list_principals() -> Result<Vec<Principal>, String> {
    with_auth(|auth| {
        auth.list_principals()
            .map_err(|e| format!("Failed to list principals: {}", e))
    })
}

/// Ensure a principal is authorized
pub fn ensure_authorized(principal: Principal) -> Result<(), String> {
    with_auth(|auth| {
        auth.ensure_authorized(principal)
            .map_err(|e| format!("Failed to ensure authorization: {}", e))
    })
}

// ═══════════════════════════════════════════════════════════════
//  Serialization Utilities (for upgrade persistence)
// ═══════════════════════════════════════════════════════════════

/// Save auth principals to bytes for stable storage
pub fn save_to_bytes() -> Vec<u8> {
    with_auth(|auth| {
        let principals = auth.list_principals().unwrap_or_default();
        candid::encode_args((&principals,)).unwrap_or_default()
    })
}

/// Load auth principals from bytes (for post-upgrade)
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

/// Validate a principal text string
pub fn validate_principal_text(text: &str) -> Result<Principal, AuthError> {
    Principal::from_text(text).map_err(|_| AuthError::InvalidPrincipal)
}

// ═══════════════════════════════════════════════════════════════
//  IC CDK Exported Functions (Optional - for standalone use)
// ═══════════════════════════════════════════════════════════════

/// Query to list authorized principals (guarded)
#[ic_cdk::query(guard = "is_authorized")]
pub fn get_authorized_principals() -> Vec<Principal> {
    list_principals().unwrap_or_default()
}

/// Update to add an authorized principal (guarded)
#[ic_cdk::update(guard = "is_authorized")]
pub fn authorize_principal(principal: Principal) {
    let _ = add_principal(principal);
}

/// Update to remove an authorized principal (guarded)
#[ic_cdk::update(guard = "is_authorized")]
pub fn deauthorize_principal(principal: Principal) -> String {
    remove_principal(principal).unwrap_or_else(|e| format!("Error: {}", e))
}

/// Query to check if a principal is authorized (guarded)
#[ic_cdk::query(guard = "is_authorized")]
pub fn check_principal_authorized(principal: Principal) -> bool {
    is_principal_authorized(principal).unwrap_or(false)
}

/// Query to get count of authorized principals (guarded)
#[ic_cdk::query(guard = "is_authorized")]
pub fn get_authorized_count() -> usize {
    list_principals().map(|list| list.len()).unwrap_or(0)
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