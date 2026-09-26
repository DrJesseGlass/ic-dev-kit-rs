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

use ic_dev_kit_rs::candle::{CandleModel, ModelMetadata};
use ic_dev_kit_rs::model_server::ModelServer;
use ic_dev_kit_rs::storage::StorageRegistry;
use ic_dev_kit_rs::text_generation::{AutoregressiveModel, GenerationConfig, TokenizerHandle};
use std::cell::RefCell;
use std::collections::HashMap;

struct DummyModel;

impl CandleModel for DummyModel {
    fn load(_weights: Vec<u8>, _config: Option<Vec<u8>>) -> Result<Self, String> {
        Ok(Self)
    }

    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: "dummy".to_string(),
            version: "0".to_string(),
            architecture: "test".to_string(),
            parameters: 0,
            context_length: None,
        }
    }

    fn reset(&mut self) {}
}

impl AutoregressiveModel for DummyModel {
    fn init_generation(
        &mut self,
        _prompt: String,
        _tokenizer: &dyn TokenizerHandle,
        _config: &GenerationConfig,
    ) -> Result<String, String> {
        Ok(String::new())
    }

    fn generate_next_token(&mut self, _tokenizer: &dyn TokenizerHandle) -> Result<String, String> {
        Ok(String::new())
    }

    fn is_generation_complete(&self) -> bool {
        true
    }

    fn generated_token_count(&self) -> usize {
        0
    }
}

struct DummyTokenizer;

impl TokenizerHandle for DummyTokenizer {
    fn encode(&self, _text: &str) -> Result<Vec<u32>, String> {
        Ok(Vec::new())
    }

    fn decode(&self, _tokens: &[u32]) -> Result<String, String> {
        Ok(String::new())
    }

    fn vocab_size(&self) -> usize {
        0
    }
}

#[derive(Default)]
struct MapRegistry(HashMap<String, Vec<u8>>);

impl StorageRegistry for MapRegistry {
    fn insert(&mut self, key: String, value: Vec<u8>) {
        self.0.insert(key, value);
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.0.get(key).cloned()
    }

    fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
        self.0.remove(key)
    }
}

thread_local! {
    static SERVER: ModelServer<DummyModel> = const { ModelServer::new() };
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
    get_tokenizer: |_model| Box::new(DummyTokenizer)
);

ic_cdk::export_candid!();

#[test]
fn all_macro_endpoints_export_to_candid() {
    let did = __export_service();

    for method in [
        // auth
        "authorize_principal",
        "deauthorize_principal",
        "get_authorized_principals",
        // telemetry (Canistergeek camelCase names plus our own)
        "getCanistergeekInformation",
        "updateCanistergeekInformation",
        "getCanisterLog",
        "authorize_monitoring",
        // uploads + storage integration
        "append_chunk",
        "append_parallel_chunk",
        "save_buffer_to_storage",
        "storage_key_exists",
        // model server
        "setup_model",
        "generate",
        "reset_generation",
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
