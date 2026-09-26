// Shared test fixture: a no-op LLM and tokenizer satisfying the ML traits.
//
// Pulled into doctests and integration tests with
// `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dummy_llm.rs"));`
// so the trait boilerplate is written once. Not a test target itself
// (cargo only discovers `tests/*.rs` and `tests/*/main.rs`).

struct DummyLlm;

impl ic_dev_kit_rs::candle::CandleModel for DummyLlm {
    fn load(_weights: Vec<u8>, _config: Option<Vec<u8>>) -> Result<Self, String> {
        Ok(DummyLlm)
    }

    fn metadata(&self) -> ic_dev_kit_rs::candle::ModelMetadata {
        ic_dev_kit_rs::candle::ModelMetadata {
            name: "dummy".to_string(),
            version: "0".to_string(),
            architecture: "test".to_string(),
            parameters: 0,
            context_length: None,
        }
    }

    fn reset(&mut self) {}
}

impl ic_dev_kit_rs::text_generation::AutoregressiveModel for DummyLlm {
    fn init_generation(
        &mut self,
        _prompt: String,
        _tokenizer: &dyn ic_dev_kit_rs::text_generation::TokenizerHandle,
        _config: &ic_dev_kit_rs::text_generation::GenerationConfig,
    ) -> Result<String, String> {
        Ok(String::new())
    }

    fn generate_next_token(
        &mut self,
        _tokenizer: &dyn ic_dev_kit_rs::text_generation::TokenizerHandle,
    ) -> Result<String, String> {
        Ok(String::new())
    }

    fn is_generation_complete(&self) -> bool {
        true
    }

    fn generated_token_count(&self) -> usize {
        0
    }
}

impl DummyLlm {
    fn tokenizer_handle(&self) -> DummyTokenizer {
        DummyTokenizer
    }
}

struct DummyTokenizer;

impl ic_dev_kit_rs::text_generation::TokenizerHandle for DummyTokenizer {
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
