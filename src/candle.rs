// ... existing code ...

// Add this helper for common GGUF loading pattern
pub mod gguf {
    use candle_core::Device;
    use candle_core::quantized::gguf_file;
    use std::io::Cursor;

    /// Load GGUF content from bytes
    pub fn load_content(weights: Vec<u8>) -> Result<(gguf_file::Content, Cursor<Vec<u8>>), String> {
        let mut cursor = Cursor::new(weights);
        let content = gguf_file::Content::read(&mut cursor)
            .map_err(|e| format!("Failed to read GGUF: {}", e))?;
        Ok((content, cursor))
    }

    /// Get CPU device (helper)
    pub fn cpu_device() -> Device {
        Device::Cpu
    }
}

// Add tokenizer helper
pub mod tokenizers {
    use tokenizers::Tokenizer;

    /// Find EOS token from common names
    pub fn find_eos_token(tokenizer: &Tokenizer) -> u32 {
        tokenizer.get_vocab(true)
            .get("<|endoftext|>")
            .or_else(|| tokenizer.get_vocab(true).get("<|im_end|>"))
            .or_else(|| tokenizer.get_vocab(true).get("</s>"))
            .or_else(|| tokenizer.get_vocab(true).get("<eos>"))
            .copied()
            .unwrap_or(0)
    }
}