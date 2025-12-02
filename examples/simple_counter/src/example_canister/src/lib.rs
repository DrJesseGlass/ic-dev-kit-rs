// Minimal example canister using ic-dev-kit-rs
// Demonstrates: auth, telemetry, storage, and a simple counter

use std::cell::RefCell;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    DefaultMemoryImpl, StableBTreeMap,
};

type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static STORAGE: RefCell<StableBTreeMap<String, Vec<u8>, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))))
    );

    static COUNTER: RefCell<u64> = RefCell::new(0);
}

// ═══════════════════════════════════════════════════════════════
//  Auth Functions
// ═══════════════════════════════════════════════════════════════

// Must call auth macro first (defines is_authorized guard)
ic_dev_kit_rs::export_auth_endpoints!();

// ═══════════════════════════════════════════════════════════════
//  Telemetry Functions
// ═══════════════════════════════════════════════════════════════

// Then telemetry macro (uses is_authorized for admin operations)
ic_dev_kit_rs::export_telemetry_endpoints!();

// ═══════════════════════════════════════════════════════════════
//  Counter Functions (Main Feature)
// ═══════════════════════════════════════════════════════════════

#[ic_cdk::update]
fn increment() -> u64 {
    // Track metrics
    ic_dev_kit_rs::telemetry::collect_metrics();

    let new_value = COUNTER.with(|c| {
        let mut counter = c.borrow_mut();
        *counter += 1;
        *counter
    });

    // Log the increment
    ic_dev_kit_rs::telemetry::log_info(&format!("Counter incremented to {}", new_value));

    // Save to stable storage
    STORAGE.with(|s| {
        ic_dev_kit_rs::storage::save_bytes(s, "counter", new_value.to_le_bytes().to_vec());
    });

    new_value
}

#[ic_cdk::query]
fn get_counter() -> u64 {
    COUNTER.with(|c| *c.borrow())
}

#[ic_cdk::update(guard = "is_authorized")]
fn reset_counter() -> String {
    ic_dev_kit_rs::telemetry::log_warning("Counter reset by admin");

    COUNTER.with(|c| {
        *c.borrow_mut() = 0;
    });

    STORAGE.with(|s| {
        ic_dev_kit_rs::storage::save_bytes(s, "counter", 0u64.to_le_bytes().to_vec());
    });

    "Counter reset to 0".to_string()
}

// ═══════════════════════════════════════════════════════════════
//  Storage Demo Functions
// ═══════════════════════════════════════════════════════════════

#[ic_cdk::update(guard = "is_authorized")]
fn store_message(key: String, message: String) -> String {
    ic_dev_kit_rs::telemetry::log_info(&format!("Storing message: {}", key));

    STORAGE.with(|s| {
        ic_dev_kit_rs::storage::save_bytes(s, &key, message.into_bytes());
    });

    format!("Stored message under key: {}", key)
}

#[ic_cdk::query]
fn get_message(key: String) -> Option<String> {
    STORAGE.with(|s| {
        ic_dev_kit_rs::storage::load_bytes(s, &key)
            .and_then(|bytes| String::from_utf8(bytes).ok())
    })
}

#[ic_cdk::query]
fn list_storage_keys() -> Vec<String> {
    // Simple implementation - in production you'd want a better index
    vec!["counter".to_string()] // Just show the counter for now
}

// We add a simple status query
#[ic_cdk::query]
fn canister_status() -> String {
    let counter = get_counter();
    let admin_count = ic_dev_kit_rs::auth::list_principals()
        .map(|p| p.len())
        .unwrap_or(0);

    format!(
        "Counter: {}\nAdmins: {}\nCaller: {}",
        counter,
        admin_count,
        ic_cdk::api::msg_caller()
    )
}

// ═══════════════════════════════════════════════════════════════
//  Lifecycle Hooks
// ═══════════════════════════════════════════════════════════════

#[ic_cdk::init]
fn init() {
    // Initialize all modules
    ic_dev_kit_rs::auth::init_with_caller();
    ic_dev_kit_rs::telemetry::init();

    ic_dev_kit_rs::telemetry::log_info("Example canister initialized");
    ic_dev_kit_rs::telemetry::log_info(&format!("Deployer: {}", ic_cdk::api::msg_caller()));
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    STORAGE.with(|s| {
        // Restore auth
        let auth_bytes = ic_dev_kit_rs::storage::load_bytes(s, "__auth__");
        ic_dev_kit_rs::auth::init_from_saved(auth_bytes);

        // Restore counter
        if let Some(counter) = ic_dev_kit_rs::storage::load_candid::<u64, _>(s, "counter") {
            COUNTER.with(|c| *c.borrow_mut() = counter);
        }
    });

    ic_dev_kit_rs::telemetry::init();
    ic_dev_kit_rs::telemetry::log_info("Example canister upgraded");
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    // Save auth state
    let auth_bytes = ic_dev_kit_rs::auth::save_to_bytes();
    STORAGE.with(|s| {
        ic_dev_kit_rs::storage::save_bytes(s, "__auth__", auth_bytes);
    });

    // Counter is already saved on each increment
    ic_dev_kit_rs::telemetry::log_info("Example canister pre-upgrade complete");
}

ic_cdk::export_candid!();