// Large object upload system with chunked buffers
//
// This module provides utilities for uploading large files to IC canisters
// using either sequential or parallel chunk uploads.
//
// ## Usage Patterns
//
// ### Sequential Upload (simple)
// ```rust
// use ic_dev_kit_rs::large_objects;
//
// // Client uploads chunks sequentially
// for chunk in file_chunks {
//     large_objects::append_chunk(chunk);
// }
//
// // Get the complete data
// let data = large_objects::get_buffer_data();
// // ... save to your REGISTRIES or process
// ```
//
// ### Parallel Upload (faster, out-of-order chunks)
// ```rust
// // Client uploads chunks in parallel (any order)
// large_objects::append_parallel_chunk(0, chunk_0);
// large_objects::append_parallel_chunk(2, chunk_2);
// large_objects::append_parallel_chunk(1, chunk_1);
//
// // Check completeness
// if large_objects::parallel_chunks_complete(3) {
//     // Get consolidated data
//     let data = large_objects::get_parallel_data()?;
//     // ... save to your REGISTRIES
// }
// ```

use std::cell::RefCell;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
//  Thread-Local Buffers
// ═══════════════════════════════════════════════════════════════

thread_local! {
    /// Single sequential buffer for simple uploads
    static BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::new());

    /// Map of chunk_id -> data for parallel uploads
    static BUFFER_MAP: RefCell<HashMap<u32, Vec<u8>>> = RefCell::new(HashMap::new());
}

// ═══════════════════════════════════════════════════════════════
//  Sequential Buffer API
// ═══════════════════════════════════════════════════════════════

/// Append a chunk to the sequential buffer
///
/// Use this for simple, ordered uploads where chunks arrive sequentially.
pub fn append_chunk(chunk: Vec<u8>) {
    BUFFER.with(|buffer| {
        buffer.borrow_mut().extend(chunk);
    });
}

/// Get current buffer size
pub fn buffer_size() -> usize {
    BUFFER.with(|buffer| buffer.borrow().len())
}

/// Clear the sequential buffer
pub fn clear_buffer() {
    BUFFER.with(|buffer| {
        buffer.borrow_mut().clear();
    });
}

/// Get buffered data (consumes the buffer)
pub fn get_buffer_data() -> Vec<u8> {
    BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        std::mem::take(&mut *buffer)
    })
}

/// Load data into the sequential buffer
pub fn load_to_buffer(data: Vec<u8>) {
    BUFFER.with(|buffer| {
        *buffer.borrow_mut() = data;
    });
}

// ═══════════════════════════════════════════════════════════════
//  Parallel Buffer API
// ═══════════════════════════════════════════════════════════════

/// Append a chunk with ID for parallel uploads
///
/// Chunks can arrive in any order. Use chunk IDs to track which chunks
/// have been received.
pub fn append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow_mut().insert(chunk_id, chunk);
    });
}

/// Get number of chunks in the parallel buffer
pub fn parallel_chunk_count() -> usize {
    BUFFER_MAP.with(|buffer_map| buffer_map.borrow().len())
}

/// Get list of chunk IDs currently in the parallel buffer
pub fn parallel_chunk_ids() -> Vec<u32> {
    BUFFER_MAP.with(|buffer_map| {
        let mut ids: Vec<u32> = buffer_map.borrow().keys().copied().collect();
        ids.sort();
        ids
    })
}

/// Get total size of all chunks in parallel buffer
pub fn parallel_buffer_size() -> usize {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow().values().map(|chunk| chunk.len()).sum()
    })
}

/// Check if all chunks from 0 to expected_count-1 are present
///
/// Returns true only if we have exactly `expected_count` chunks
/// numbered consecutively from 0.
pub fn parallel_chunks_complete(expected_count: u32) -> bool {
    BUFFER_MAP.with(|buffer_map| {
        let buffer_map = buffer_map.borrow();

        if buffer_map.len() != expected_count as usize {
            return false;
        }

        // Check that we have consecutive chunks from 0 to expected_count-1
        for i in 0..expected_count {
            if !buffer_map.contains_key(&i) {
                return false;
            }
        }

        true
    })
}

/// Check which chunks are missing (if any)
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

/// Consolidate parallel chunks into the sequential buffer
///
/// This moves data from BUFFER_MAP to BUFFER in chunk ID order,
/// then clears BUFFER_MAP.
///
/// Returns the total size of consolidated data.
pub fn consolidate_parallel_chunks() -> Result<usize, String> {
    let (chunk_data, total_size) = BUFFER_MAP.with(|buffer_map| {
        let mut buffer_map = buffer_map.borrow_mut();

        if buffer_map.is_empty() {
            return (Vec::new(), 0);
        }

        // Sort chunk IDs and collect data in order
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

        // Clear the map after consolidation
        buffer_map.clear();

        (consolidated_data, total_size)
    });

    if chunk_data.is_empty() {
        return Err("No parallel chunks to consolidate".to_string());
    }

    // Move consolidated data to main buffer
    BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        buffer.clear(); // Clear existing buffer
        buffer.extend(chunk_data);
    });

    Ok(total_size)
}

/// Get consolidated data from parallel chunks (without moving to BUFFER)
///
/// Returns the data in chunk ID order. Does NOT clear the parallel buffer.
pub fn get_parallel_data() -> Result<Vec<u8>, String> {
    BUFFER_MAP.with(|buffer_map| {
        let buffer_map = buffer_map.borrow();

        if buffer_map.is_empty() {
            return Err("No parallel chunks available".to_string());
        }

        // Sort chunk IDs and collect data in order
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

/// Clear all parallel chunks
pub fn clear_parallel_chunks() {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow_mut().clear();
    });
}

/// Remove a specific chunk from parallel buffer
///
/// Useful for retry scenarios where a chunk needs to be re-uploaded.
pub fn remove_parallel_chunk(chunk_id: u32) -> bool {
    BUFFER_MAP.with(|buffer_map| {
        buffer_map.borrow_mut().remove(&chunk_id).is_some()
    })
}

// ═══════════════════════════════════════════════════════════════
//  Storage Status and Monitoring
// ═══════════════════════════════════════════════════════════════

/// Get detailed storage status
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

#[derive(Debug, Clone)]
pub struct StorageStatus {
    pub buffer_size: usize,
    pub parallel_chunk_count: usize,
    pub parallel_buffer_size: usize,
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
        assert_eq!(buffer_size(), 0); // Buffer consumed
    }

    #[test]
    fn test_parallel_chunks() {
        clear_parallel_chunks();

        // Upload chunks out of order
        append_parallel_chunk(2, vec![5, 6]);
        append_parallel_chunk(0, vec![1, 2]);
        append_parallel_chunk(1, vec![3, 4]);

        assert_eq!(parallel_chunk_count(), 3);
        assert_eq!(parallel_buffer_size(), 6);

        // Check completeness
        assert!(parallel_chunks_complete(3));
        assert!(!parallel_chunks_complete(4));

        // Get IDs (should be sorted)
        assert_eq!(parallel_chunk_ids(), vec![0, 1, 2]);
    }

    #[test]
    fn test_consolidate_parallel_chunks() {
        clear_buffer();
        clear_parallel_chunks();

        // Upload chunks
        append_parallel_chunk(1, vec![3, 4]);
        append_parallel_chunk(0, vec![1, 2]);
        append_parallel_chunk(2, vec![5, 6]);

        // Consolidate
        let size = consolidate_parallel_chunks().unwrap();
        assert_eq!(size, 6);

        // Check sequential buffer has data in correct order
        assert_eq!(buffer_size(), 6);
        let data = get_buffer_data();
        assert_eq!(data, vec![1, 2, 3, 4, 5, 6]);

        // Parallel chunks should be cleared
        assert_eq!(parallel_chunk_count(), 0);
    }

    #[test]
    fn test_missing_chunks() {
        clear_parallel_chunks();

        append_parallel_chunk(0, vec![1, 2]);
        append_parallel_chunk(2, vec![5, 6]);
        // Missing chunk 1

        let missing = missing_chunks(3);
        assert_eq!(missing, vec![1]);

        assert!(!parallel_chunks_complete(3));
    }

    #[test]
    fn test_remove_parallel_chunk() {
        clear_parallel_chunks();

        append_parallel_chunk(0, vec![1, 2]);
        append_parallel_chunk(1, vec![3, 4]);

        assert!(remove_parallel_chunk(0));
        assert!(!remove_parallel_chunk(0)); // Already removed

        assert_eq!(parallel_chunk_count(), 1);
    }

    #[test]
    fn test_storage_status() {
        clear_buffer();
        clear_parallel_chunks();

        append_chunk(vec![1, 2, 3]);
        append_parallel_chunk(0, vec![4, 5]);
        append_parallel_chunk(1, vec![6, 7, 8]);

        let status = storage_status();
        assert_eq!(status.buffer_size, 3);
        assert_eq!(status.parallel_chunk_count, 2);
        assert_eq!(status.parallel_buffer_size, 5);
        assert_eq!(status.parallel_chunk_ids, vec![0, 1]);
    }
}