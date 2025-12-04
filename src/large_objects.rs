//! Large object upload system with chunked buffers.
//!
//! This module provides utilities for uploading large files to IC canisters
//! using either sequential or parallel chunk uploads.
//!
//! # Sequential Uploads
//!
//! For simple use cases where chunks arrive in order:
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::large_objects;
//!
//! #[ic_cdk::update]
//! fn upload_chunk(data: Vec<u8>) {
//!     large_objects::append_chunk(data);
//! }
//!
//! #[ic_cdk::update]
//! fn finalize() -> Vec<u8> {
//!     large_objects::get_buffer_data()
//! }
//! ```
//!
//! # Parallel Uploads
//!
//! For faster uploads where chunks may arrive out of order:
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::large_objects;
//!
//! #[ic_cdk::update]
//! fn upload_parallel(chunk_id: u32, data: Vec<u8>) {
//!     large_objects::append_parallel_chunk(chunk_id, data);
//! }
//!
//! #[ic_cdk::query]
//! fn is_complete(expected: u32) -> bool {
//!     large_objects::parallel_chunks_complete(expected)
//! }
//!
//! #[ic_cdk::update]
//! fn finalize() -> Result<Vec<u8>, String> {
//!     large_objects::consolidate_parallel_chunks()?;
//!     Ok(large_objects::get_buffer_data())
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
//  Thread-Local Buffers
// ═══════════════════════════════════════════════════════════════

thread_local! {
    /// Single sequential buffer for simple uploads.
    static BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::new());

    /// Map of chunk_id -> data for parallel uploads.
    static BUFFER_MAP: RefCell<HashMap<u32, Vec<u8>>> = RefCell::new(HashMap::new());
}

// ═══════════════════════════════════════════════════════════════
//  Sequential Buffer API
// ═══════════════════════════════════════════════════════════════

/// Append a chunk to the sequential buffer.
///
/// Chunks are concatenated in the order they are received.
pub fn append_chunk(chunk: Vec<u8>) {
    BUFFER.with(|buffer| {
        buffer.borrow_mut().extend(chunk);
    });
}

/// Get the current size of the sequential buffer in bytes.
pub fn buffer_size() -> usize {
    BUFFER.with(|buffer| buffer.borrow().len())
}

/// Clear the sequential buffer.
pub fn clear_buffer() {
    BUFFER.with(|buffer| {
        buffer.borrow_mut().clear();
    });
}

/// Get and consume the buffered data.
///
/// Returns all data from the sequential buffer and clears it.
pub fn get_buffer_data() -> Vec<u8> {
    BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        std::mem::take(&mut *buffer)
    })
}

/// Load data into the sequential buffer, replacing any existing data.
pub fn load_to_buffer(data: Vec<u8>) {
    BUFFER.with(|buffer| {
        *buffer.borrow_mut() = data;
    });
}

// ═══════════════════════════════════════════════════════════════
//  Parallel Buffer API
// ═══════════════════════════════════════════════════════════════

/// Append a chunk with a specific ID for parallel uploads.
///
/// Chunks can arrive in any order. Use [`consolidate_parallel_chunks`] to
/// combine them in order after all chunks are received.
///
/// # Arguments
///
/// * `chunk_id` - Zero-based chunk index (0, 1, 2, ...)
/// * `chunk` - The chunk data
pub fn append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow_mut().insert(chunk_id, chunk);
    });
}

/// Get the number of chunks in the parallel buffer.
pub fn parallel_chunk_count() -> usize {
    BUFFER_MAP.with(|buffer_map| buffer_map.borrow().len())
}

/// Get a sorted list of chunk IDs currently in the parallel buffer.
pub fn parallel_chunk_ids() -> Vec<u32> {
    BUFFER_MAP.with(|buffer_map| {
        let mut ids: Vec<u32> = buffer_map.borrow().keys().copied().collect();
        ids.sort();
        ids
    })
}

/// Get the total size of all chunks in the parallel buffer.
pub fn parallel_buffer_size() -> usize {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow().values().map(|chunk| chunk.len()).sum()
    })
}

/// Check if all chunks from 0 to expected_count-1 are present.
///
/// # Arguments
///
/// * `expected_count` - The total number of chunks expected
///
/// # Returns
///
/// `true` if chunks 0, 1, 2, ..., expected_count-1 are all present.
pub fn parallel_chunks_complete(expected_count: u32) -> bool {
    BUFFER_MAP.with(|buffer_map| {
        let buffer_map = buffer_map.borrow();

        if buffer_map.len() != expected_count as usize {
            return false;
        }

        for i in 0..expected_count {
            if !buffer_map.contains_key(&i) {
                return false;
            }
        }

        true
    })
}

/// Check which chunk IDs are missing.
///
/// # Arguments
///
/// * `expected_count` - The total number of chunks expected
///
/// # Returns
///
/// A list of missing chunk IDs (0-indexed).
pub fn missing_chunks(expected_count: u32) -> Vec<u32> {
    BUFFER_MAP.with(|buffer_map| {
        let buffer_map = buffer_map.borrow();
        let mut missing = Vec::new();

        for i in 0..expected_count {
            if !buffer_map.contains_key(&i) {
                missing.push(i);
            }
        }

        missing
    })
}

/// Consolidate parallel chunks into the sequential buffer.
///
/// Combines all parallel chunks in order (by chunk_id) and moves the result
/// to the sequential buffer. Clears the parallel buffer.
///
/// # Returns
///
/// The total size of consolidated data, or an error if no chunks are present.
///
/// # Errors
///
/// Returns an error if the parallel buffer is empty.
pub fn consolidate_parallel_chunks() -> Result<usize, String> {
    let (chunk_data, total_size) = BUFFER_MAP.with(|buffer_map| {
        let mut buffer_map = buffer_map.borrow_mut();

        if buffer_map.is_empty() {
            return (Vec::new(), 0);
        }

        let mut sorted_ids: Vec<u32> = buffer_map.keys().copied().collect();
        sorted_ids.sort();

        let mut consolidated_data = Vec::new();
        let mut total_size = 0;

        for chunk_id in sorted_ids {
            if let Some(chunk) = buffer_map.remove(&chunk_id) {
                total_size += chunk.len();
                consolidated_data.extend(chunk);
            }
        }

        buffer_map.clear();

        (consolidated_data, total_size)
    });

    if chunk_data.is_empty() {
        return Err("No parallel chunks to consolidate".to_string());
    }

    BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        buffer.clear();
        buffer.extend(chunk_data);
    });

    Ok(total_size)
}

/// Get consolidated data from parallel chunks without moving to the sequential buffer.
///
/// Combines chunks in order but leaves them in the parallel buffer.
///
/// # Errors
///
/// Returns an error if the parallel buffer is empty.
pub fn get_parallel_data() -> Result<Vec<u8>, String> {
    BUFFER_MAP.with(|buffer_map| {
        let buffer_map = buffer_map.borrow();

        if buffer_map.is_empty() {
            return Err("No parallel chunks available".to_string());
        }

        let mut sorted_ids: Vec<u32> = buffer_map.keys().copied().collect();
        sorted_ids.sort();

        let mut consolidated_data = Vec::new();

        for chunk_id in sorted_ids {
            if let Some(chunk) = buffer_map.get(&chunk_id) {
                consolidated_data.extend_from_slice(chunk);
            }
        }

        Ok(consolidated_data)
    })
}

/// Clear all parallel chunks.
pub fn clear_parallel_chunks() {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow_mut().clear();
    });
}

/// Remove a specific chunk from the parallel buffer.
///
/// # Returns
///
/// `true` if the chunk was present and removed.
pub fn remove_parallel_chunk(chunk_id: u32) -> bool {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow_mut().remove(&chunk_id).is_some()
    })
}

// ═══════════════════════════════════════════════════════════════
//  Storage Status and Monitoring
// ═══════════════════════════════════════════════════════════════

/// Get detailed status of both buffers.
pub fn storage_status() -> StorageStatus {
    let buffer_size = buffer_size();

    let (chunk_count, parallel_size, chunk_ids) = BUFFER_MAP.with(|buffer_map| {
        let buffer_map = buffer_map.borrow();
        let count = buffer_map.len();
        let size = buffer_map.values().map(|chunk| chunk.len()).sum::<usize>();
        let mut ids: Vec<u32> = buffer_map.keys().copied().collect();
        ids.sort();
        (count, size, ids)
    });

    StorageStatus {
        buffer_size,
        parallel_chunk_count: chunk_count,
        parallel_buffer_size: parallel_size,
        parallel_chunk_ids: chunk_ids,
    }
}

/// Status information for upload buffers.
#[derive(Debug, Clone)]
pub struct StorageStatus {
    /// Size of the sequential buffer in bytes.
    pub buffer_size: usize,
    /// Number of chunks in the parallel buffer.
    pub parallel_chunk_count: usize,
    /// Total size of all parallel chunks in bytes.
    pub parallel_buffer_size: usize,
    /// Sorted list of chunk IDs in the parallel buffer.
    pub parallel_chunk_ids: Vec<u32>,
}

impl std::fmt::Display for StorageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sequential buffer: {} bytes\n\
             Parallel chunks: {} chunks, {} bytes total\n\
             Chunk IDs: {:?}",
            self.buffer_size,
            self.parallel_chunk_count,
            self.parallel_buffer_size,
            self.parallel_chunk_ids
        )
    }
}

// ═══════════════════════════════════════════════════════════════
//  MACRO: Auto-generate IC endpoints
// ═══════════════════════════════════════════════════════════════

/// Generate IC endpoints for large object uploads.
///
/// This macro creates endpoints for both sequential and parallel uploads,
/// with optional storage integration.
///
/// # Basic Usage (upload only)
///
/// ```rust,ignore
/// ic_dev_kit_rs::generate_upload_endpoints!(guard = "auth::is_authorized");
/// ```
///
/// # With Storage Integration
///
/// ```rust,ignore
/// ic_dev_kit_rs::generate_upload_endpoints!(
///     guard = "auth::is_authorized",
///     registry = REGISTRIES
/// );
/// ```
///
/// # Generated Endpoints
///
/// **Sequential uploads:**
/// - `append_chunk(chunk: Vec<u8>) -> usize`
/// - `buffer_size() -> usize`
/// - `clear_buffer()`
///
/// **Parallel uploads:**
/// - `append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) -> usize`
/// - `parallel_chunks_complete(expected_count: u32) -> bool`
/// - `missing_chunks(expected_count: u32) -> Vec<u32>`
/// - `clear_parallel_chunks()`
/// - `parallel_chunk_count() -> usize`
///
/// **Storage (if registry provided):**
/// - `save_buffer_to_storage(key: String) -> Result<String, String>`
/// - `save_parallel_to_storage(key: String) -> Result<String, String>`
/// - `storage_key_exists(key: String) -> bool`
/// - `get_storage_size(key: String) -> Option<usize>`
/// - `delete_storage_key(key: String) -> Result<String, String>`
///
/// **Status:**
/// - `get_storage_status() -> String`
#[macro_export]
macro_rules! generate_upload_endpoints {
    // With storage registry
    (guard = $guard:expr, registry = $registry:ident) => {
        // Sequential upload endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn append_chunk(chunk: Vec<u8>) -> usize {
            $crate::large_objects::append_chunk(chunk);
            $crate::large_objects::buffer_size()
        }

        #[ic_cdk::query]
        pub fn buffer_size() -> usize {
            $crate::large_objects::buffer_size()
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn clear_buffer() {
            $crate::large_objects::clear_buffer();
        }

        // Parallel upload endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) -> usize {
            $crate::large_objects::append_parallel_chunk(chunk_id, chunk);
            $crate::large_objects::parallel_chunk_count()
        }

        #[ic_cdk::query]
        pub fn parallel_chunks_complete(expected_count: u32) -> bool {
            $crate::large_objects::parallel_chunks_complete(expected_count)
        }

        #[ic_cdk::query]
        pub fn missing_chunks(expected_count: u32) -> Vec<u32> {
            $crate::large_objects::missing_chunks(expected_count)
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn clear_parallel_chunks() {
            $crate::large_objects::clear_parallel_chunks();
        }

        #[ic_cdk::query]
        pub fn parallel_chunk_count() -> usize {
            $crate::large_objects::parallel_chunk_count()
        }

        // Storage integration endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn save_buffer_to_storage(key: String) -> Result<String, String> {
            let data = $crate::large_objects::get_buffer_data();
            if data.is_empty() {
                return Err("No data in buffer".to_string());
            }

            let size = data.len();
            $registry.with(|r| {
                $crate::storage::save_bytes(r, &key, data);
            });

            $crate::large_objects::clear_buffer();

            #[cfg(feature = "telemetry")]
            $crate::telemetry::log_info(&format!("Saved {} bytes to key '{}'", size, key));

            Ok(format!("Saved {} bytes to key '{}'", size, key))
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn save_parallel_to_storage(key: String) -> Result<String, String> {
            let data = $crate::large_objects::get_parallel_data()?;
            let size = data.len();

            $registry.with(|r| {
                $crate::storage::save_bytes(r, &key, data);
            });

            $crate::large_objects::clear_parallel_chunks();

            #[cfg(feature = "telemetry")]
            $crate::telemetry::log_info(&format!("Saved {} bytes to key '{}'", size, key));

            Ok(format!("Saved {} bytes to key '{}'", size, key))
        }

        #[ic_cdk::query]
        pub fn storage_key_exists(key: String) -> bool {
            $registry.with(|r| $crate::storage::exists(r, &key))
        }

        #[ic_cdk::query]
        pub fn get_storage_size(key: String) -> Option<usize> {
            $registry.with(|r| $crate::storage::size(r, &key))
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn delete_storage_key(key: String) -> Result<String, String> {
            let deleted = $registry.with(|r| $crate::storage::delete(r, &key));

            if deleted {
                Ok(format!("Deleted key '{}'", key))
            } else {
                Err(format!("Key '{}' not found", key))
            }
        }

        // Status endpoint
        #[ic_cdk::query]
        pub fn get_storage_status() -> String {
            $crate::large_objects::storage_status().to_string()
        }
    };

    // Without storage registry (just upload endpoints)
    (guard = $guard:expr) => {
        // Sequential upload endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn append_chunk(chunk: Vec<u8>) -> usize {
            $crate::large_objects::append_chunk(chunk);
            $crate::large_objects::buffer_size()
        }

        #[ic_cdk::query]
        pub fn buffer_size() -> usize {
            $crate::large_objects::buffer_size()
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn clear_buffer() {
            $crate::large_objects::clear_buffer();
        }

        // Parallel upload endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) -> usize {
            $crate::large_objects::append_parallel_chunk(chunk_id, chunk);
            $crate::large_objects::parallel_chunk_count()
        }

        #[ic_cdk::query]
        pub fn parallel_chunks_complete(expected_count: u32) -> bool {
            $crate::large_objects::parallel_chunks_complete(expected_count)
        }

        #[ic_cdk::query]
        pub fn missing_chunks(expected_count: u32) -> Vec<u32> {
            $crate::large_objects::missing_chunks(expected_count)
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn clear_parallel_chunks() {
            $crate::large_objects::clear_parallel_chunks();
        }

        #[ic_cdk::query]
        pub fn parallel_chunk_count() -> usize {
            $crate::large_objects::parallel_chunk_count()
        }

        // Status endpoint
        #[ic_cdk::query]
        pub fn get_storage_status() -> String {
            $crate::large_objects::storage_status().to_string()
        }
    };

    // No guard (public endpoints - not recommended for production!)
    () => {
        ic_dev_kit_rs::generate_upload_endpoints!(guard = "|| Ok(())");
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_buffer() {
        clear_buffer();

        append_chunk(vec![1, 2, 3]);
        append_chunk(vec![4, 5, 6]);

        assert_eq!(buffer_size(), 6);

        let data = get_buffer_data();
        assert_eq!(data, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(buffer_size(), 0);
    }

    #[test]
    fn test_parallel_chunks() {
        clear_parallel_chunks();

        append_parallel_chunk(2, vec![5, 6]);
        append_parallel_chunk(0, vec![1, 2]);
        append_parallel_chunk(1, vec![3, 4]);

        assert_eq!(parallel_chunk_count(), 3);
        assert_eq!(parallel_buffer_size(), 6);
        assert!(parallel_chunks_complete(3));
        assert!(!parallel_chunks_complete(4));
        assert_eq!(parallel_chunk_ids(), vec![0, 1, 2]);
    }
}