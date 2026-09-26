// Shared test fixture: an in-memory `StorageRegistry` backed by a HashMap.
//
// Pulled into doctests and integration tests with
// `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/map_registry.rs"));`
// Not a test target itself (cargo only discovers `tests/*.rs` and
// `tests/*/main.rs`).

#[derive(Default)]
struct MapRegistry(std::collections::HashMap<String, Vec<u8>>);

impl ic_dev_kit_rs::storage::StorageRegistry for MapRegistry {
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
