//! Regression test: every endpoint-generating macro must survive
//! `ic_cdk::export_candid!`.
//!
//! `export_candid!` re-parses the *stringified* signatures of every
//! `#[ic_cdk::update]`/`#[ic_cdk::query]` function in the crate. Macro-emitted
//! signatures that contain `$crate::...` paths cannot be parsed there, which
//! broke consumers of `generate_model_endpoints!` in 0.4.0 even though the
//! macro itself type-checked. Expanding all four macros here and calling
//! `export_candid!` in the same module catches that class of bug in CI.
#![cfg(all(
    feature = "storage",
    feature = "telemetry",
    feature = "text-generation"
))]

use ic_dev_kit_rs::model_server::ModelServer;
use std::cell::RefCell;

// Shared with the doctests; see the fixture files for the trait impls.
include!("fixtures/dummy_llm.rs");
include!("fixtures/map_registry.rs");

thread_local! {
    static SERVER: ModelServer<DummyLlm> = const { ModelServer::new() };
    static REGISTRY: RefCell<MapRegistry> = RefCell::new(MapRegistry::default());
}

// The four macros, exactly as a canister would invoke them at its crate root.
ic_dev_kit_rs::export_auth_endpoints!();
ic_dev_kit_rs::export_telemetry_endpoints!();
ic_dev_kit_rs::generate_upload_endpoints!(guard = "is_authorized", registry = REGISTRY);
ic_dev_kit_rs::generate_model_endpoints!(
    server: SERVER,
    registry: REGISTRY,
    weights_key: "weights",
    tokenizer_key: "tokenizer",
    get_tokenizer: |model| Box::new(model.tokenizer_handle())
);

ic_cdk::export_candid!();

#[test]
fn all_macro_endpoints_export_to_candid() {
    let did = __export_service();

    // Every endpoint the four macros emit (see the README's Macros table).
    for method in [
        // auth
        "authorize_principal",
        "deauthorize_principal",
        "get_authorized_principals",
        "check_principal_authorized",
        "get_authorized_count",
        // telemetry (Canistergeek camelCase names plus our own)
        "getCanistergeekInformation",
        "updateCanistergeekInformation",
        "getCanisterLog",
        "authorize_monitoring",
        "deauthorize_monitoring",
        "get_monitoring_principals",
        // uploads
        "append_chunk",
        "buffer_size",
        "clear_buffer",
        "append_parallel_chunk",
        "parallel_chunks_complete",
        "missing_chunks",
        "clear_parallel_chunks",
        "parallel_chunk_count",
        "get_storage_status",
        // uploads: storage integration
        "save_buffer_to_storage",
        "save_parallel_to_storage",
        "storage_key_exists",
        "get_storage_size",
        "delete_storage_key",
        // model server
        "setup_model",
        "generate",
        "reset_generation",
        "is_model_loaded",
        "get_model_info",
    ] {
        assert!(
            did.contains(&format!("{method} :")),
            "{method} missing from exported service:\n{did}"
        );
    }

    // Spot-check the 0.4.0 type changes on the wire.
    assert!(did.contains("deauthorize_principal : (principal) -> (Result"));
    assert!(did.contains("setup_model : () -> (Result"));
}
