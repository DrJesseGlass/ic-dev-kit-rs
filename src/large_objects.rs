//! Large object upload system with chunked, per-caller buffers.
//!
//! This module provides utilities for uploading large files to IC canisters
//! using either sequential or parallel chunk uploads.
//!
//! # Isolation and limits
//!
//! Buffers are keyed by an `owner` [`Principal`], so concurrent uploads from
//! different callers cannot interleave into each other's data. The macro-
//! generated endpoints pass `ic_cdk::api::msg_caller()` as the owner
//! automatically.
//!
//! Each owner's total buffered bytes (sequential + parallel combined) is
//! capped at [`DEFAULT_MAX_BYTES_PER_OWNER`] by default; adjust with
//! [`set_max_bytes_per_owner`]. Because anyone can mint fresh principals, a
//! per-owner cap alone cannot protect the heap, so the total across all owners
//! is also capped at [`DEFAULT_MAX_TOTAL_BYTES`]; adjust with
//! [`set_max_total_bytes`].
//!
//! **Note:** buffers live on the Wasm heap, not in stable memory. An in-flight
//! upload is lost on canister upgrade — finalize uploads (e.g. save to a
//! storage registry) before upgrading.
//!
//! # Sequential Uploads
//!
//! For simple use cases where chunks arrive in order:
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::large_objects;
//!
//! #[ic_cdk::update]
//! fn upload_chunk(data: Vec<u8>) -> Result<usize, String> {
//!     large_objects::append_chunk(ic_cdk::api::msg_caller(), data)
//! }
//!
//! #[ic_cdk::update]
//! fn finalize() -> Vec<u8> {
//!     large_objects::get_buffer_data(ic_cdk::api::msg_caller())
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
//! fn upload_parallel(chunk_id: u32, data: Vec<u8>) -> Result<usize, String> {
//!     large_objects::append_parallel_chunk(ic_cdk::api::msg_caller(), chunk_id, data)
//! }
//!
//! #[ic_cdk::query]
//! fn is_complete(expected: u32) -> bool {
//!     large_objects::parallel_chunks_complete(ic_cdk::api::msg_caller(), expected)
//! }
//!
//! #[ic_cdk::update]
//! fn finalize() -> Result<Vec<u8>, String> {
//!     let owner = ic_cdk::api::msg_caller();
//!     large_objects::consolidate_parallel_chunks(owner)?;
//!     Ok(large_objects::get_buffer_data(owner))
//! }
//! ```

use candid::Principal;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Default cap on total buffered bytes per owner: 1 GiB.
///
/// The sequential buffer is a single `Vec<u8>`, and `Vec` growth doubles its
/// capacity, so a buffer approaching half the 4 GiB Wasm heap cannot grow
/// further without trapping. 1 GiB keeps the worst-case reallocation (copying
/// into a 2 GiB allocation) inside the heap. Adjust with
/// [`set_max_bytes_per_owner`].
pub const DEFAULT_MAX_BYTES_PER_OWNER: usize = 1024 * 1024 * 1024;

/// Default cap on buffered bytes summed across all owners: 2 GiB.
///
/// Leaves headroom on the 4 GiB heap for the canister's own state and for
/// consolidation copies. Adjust with [`set_max_total_bytes`].
pub const DEFAULT_MAX_TOTAL_BYTES: usize = 2 * 1024 * 1024 * 1024;

// ═══════════════════════════════════════════════════════════════
//  Thread-Local Buffers (keyed by owner principal)
// ═══════════════════════════════════════════════════════════════

thread_local! {
    /// Sequential buffers, one per owner.
    static BUFFERS: RefCell<HashMap<Principal, Vec<u8>>> = RefCell::new(HashMap::new());

    /// Parallel chunk maps (chunk_id -> data), one per owner.
    static BUFFER_MAPS: RefCell<HashMap<Principal, HashMap<u32, Vec<u8>>>> =
        RefCell::new(HashMap::new());

    /// Per-owner byte cap. `None` disables the limit.
    static MAX_BYTES_PER_OWNER: Cell<Option<usize>> = Cell::new(Some(DEFAULT_MAX_BYTES_PER_OWNER));

    /// Cap on bytes across all owners. `None` disables the limit.
    static MAX_TOTAL_BYTES: Cell<Option<usize>> = Cell::new(Some(DEFAULT_MAX_TOTAL_BYTES));
}

// ═══════════════════════════════════════════════════════════════
//  Limits
// ═══════════════════════════════════════════════════════════════

/// Set the cap on total buffered bytes (sequential + parallel) per owner.
///
/// Pass `None` to disable the limit entirely.
pub fn set_max_bytes_per_owner(limit: Option<usize>) {
    MAX_BYTES_PER_OWNER.with(|m| m.set(limit));
}

/// Get the current per-owner byte cap (`None` = unlimited).
pub fn max_bytes_per_owner() -> Option<usize> {
    MAX_BYTES_PER_OWNER.with(|m| m.get())
}

/// Set the cap on buffered bytes summed across all owners.
///
/// Pass `None` to disable the limit entirely.
pub fn set_max_total_bytes(limit: Option<usize>) {
    MAX_TOTAL_BYTES.with(|m| m.set(limit));
}

/// Get the current cap across all owners (`None` = unlimited).
pub fn max_total_bytes() -> Option<usize> {
    MAX_TOTAL_BYTES.with(|m| m.get())
}

/// Total bytes currently buffered for an owner (sequential + parallel).
pub fn total_buffered_bytes(owner: Principal) -> usize {
    buffer_size(owner) + parallel_buffer_size(owner)
}

/// Total bytes currently buffered across all owners.
pub fn total_buffered_bytes_all_owners() -> usize {
    let sequential: usize = BUFFERS.with(|b| b.borrow().values().map(Vec::len).sum());
    let parallel: usize = BUFFER_MAPS.with(|maps| {
        maps.borrow()
            .values()
            .flat_map(HashMap::values)
            .map(Vec::len)
            .sum()
    });
    sequential + parallel
}

/// Enforce both caps for a write that adds `incoming` net bytes to `owner`.
///
/// `owner_current` is the owner's usage excluding anything the write replaces;
/// the global total is measured here, and the same replaced bytes are
/// subtracted from it via `replaced`. Must be called before taking a mutable
/// borrow on the buffer maps.
fn check_capacity(owner_current: usize, replaced: usize, incoming: usize) -> Result<(), String> {
    if let Some(limit) = max_bytes_per_owner() {
        if owner_current.saturating_add(incoming) > limit {
            return Err(format!(
                "Upload buffer limit exceeded: {} buffered + {} incoming > {} byte per-owner cap",
                owner_current, incoming, limit
            ));
        }
    }
    if let Some(limit) = max_total_bytes() {
        let total = total_buffered_bytes_all_owners().saturating_sub(replaced);
        if total.saturating_add(incoming) > limit {
            return Err(format!(
                "Upload buffer limit exceeded: {} buffered across all owners + {} incoming > {} byte total cap",
                total, incoming, limit
            ));
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════
//  Sequential Buffer API
// ═══════════════════════════════════════════════════════════════

/// Append a chunk to the owner's sequential buffer.
///
/// Chunks are concatenated in the order they are received.
///
/// # Returns
///
/// The new buffer size in bytes, or an error if a cap would be exceeded.
pub fn append_chunk(owner: Principal, chunk: Vec<u8>) -> Result<usize, String> {
    if chunk.is_empty() {
        // Nothing to add; avoid creating an empty entry for the owner.
        return Ok(buffer_size(owner));
    }
    check_capacity(total_buffered_bytes(owner), 0, chunk.len())?;
    BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        let buffer = buffers.entry(owner).or_default();
        buffer.extend(chunk);
        Ok(buffer.len())
    })
}

/// Get the current size of the owner's sequential buffer in bytes.
pub fn buffer_size(owner: Principal) -> usize {
    BUFFERS.with(|buffers| buffers.borrow().get(&owner).map(Vec::len).unwrap_or(0))
}

/// Clear the owner's sequential buffer.
pub fn clear_buffer(owner: Principal) {
    BUFFERS.with(|buffers| {
        buffers.borrow_mut().remove(&owner);
    });
}

/// Get and consume the owner's buffered data.
///
/// Returns all data from the sequential buffer and clears it.
pub fn get_buffer_data(owner: Principal) -> Vec<u8> {
    BUFFERS.with(|buffers| buffers.borrow_mut().remove(&owner).unwrap_or_default())
}

/// Load data into the owner's sequential buffer, replacing any existing data.
///
/// # Errors
///
/// Returns an error if the data would exceed a cap.
pub fn load_to_buffer(owner: Principal, data: Vec<u8>) -> Result<(), String> {
    // Replaces the sequential buffer, so only parallel bytes count as current.
    check_capacity(parallel_buffer_size(owner), buffer_size(owner), data.len())?;
    BUFFERS.with(|buffers| {
        buffers.borrow_mut().insert(owner, data);
    });
    Ok(())
}

// ═══════════════════════════════════════════════════════════════
//  Parallel Buffer API
// ═══════════════════════════════════════════════════════════════

/// Append a chunk with a specific ID to the owner's parallel buffer.
///
/// Chunks can arrive in any order. Use [`consolidate_parallel_chunks`] to
/// combine them in order after all chunks are received. Re-sending an existing
/// `chunk_id` replaces that chunk.
///
/// # Arguments
///
/// * `owner` - The uploading principal (use `ic_cdk::api::msg_caller()`)
/// * `chunk_id` - Zero-based chunk index (0, 1, 2, ...)
/// * `chunk` - The chunk data
///
/// # Returns
///
/// The number of chunks now buffered for this owner, or an error if a cap
/// would be exceeded.
pub fn append_parallel_chunk(
    owner: Principal,
    chunk_id: u32,
    chunk: Vec<u8>,
) -> Result<usize, String> {
    // A replaced chunk frees its old bytes, so only the delta counts.
    let replaced = BUFFER_MAPS.with(|maps| {
        maps.borrow()
            .get(&owner)
            .and_then(|m| m.get(&chunk_id))
            .map_or(0, Vec::len)
    });
    check_capacity(
        total_buffered_bytes(owner) - replaced,
        replaced,
        chunk.len(),
    )?;
    BUFFER_MAPS.with(|maps| {
        let mut maps = maps.borrow_mut();
        let map = maps.entry(owner).or_default();
        map.insert(chunk_id, chunk);
        Ok(map.len())
    })
}

/// Get the number of chunks in the owner's parallel buffer.
pub fn parallel_chunk_count(owner: Principal) -> usize {
    BUFFER_MAPS.with(|maps| maps.borrow().get(&owner).map(HashMap::len).unwrap_or(0))
}

/// Get a sorted list of chunk IDs currently in the owner's parallel buffer.
pub fn parallel_chunk_ids(owner: Principal) -> Vec<u32> {
    BUFFER_MAPS.with(|maps| {
        let maps = maps.borrow();
        let mut ids: Vec<u32> = maps
            .get(&owner)
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default();
        ids.sort();
        ids
    })
}

/// Get the total size of all chunks in the owner's parallel buffer.
pub fn parallel_buffer_size(owner: Principal) -> usize {
    BUFFER_MAPS.with(|maps| {
        maps.borrow()
            .get(&owner)
            .map(|m| m.values().map(Vec::len).sum())
            .unwrap_or(0)
    })
}

/// Check if all of the owner's chunks from 0 to expected_count-1 are present.
pub fn parallel_chunks_complete(owner: Principal, expected_count: u32) -> bool {
    BUFFER_MAPS.with(|maps| {
        let maps = maps.borrow();
        let Some(map) = maps.get(&owner) else {
            return expected_count == 0;
        };

        map.len() == expected_count as usize && (0..expected_count).all(|i| map.contains_key(&i))
    })
}

/// Check which of the owner's chunk IDs are missing.
///
/// # Returns
///
/// A list of missing chunk IDs (0-indexed).
pub fn missing_chunks(owner: Principal, expected_count: u32) -> Vec<u32> {
    BUFFER_MAPS.with(|maps| {
        let maps = maps.borrow();
        match maps.get(&owner) {
            Some(map) => (0..expected_count).filter(|i| !map.contains_key(i)).collect(),
            None => (0..expected_count).collect(),
        }
    })
}

/// Consolidate the owner's parallel chunks into their sequential buffer.
///
/// Combines all parallel chunks in order (by chunk_id) and moves the result
/// to the sequential buffer, replacing its previous contents. Clears the
/// parallel buffer.
///
/// # Returns
///
/// The total size of consolidated data.
///
/// # Errors
///
/// Returns an error if the owner's parallel buffer is empty; the buffers are
/// left untouched in that case.
pub fn consolidate_parallel_chunks(owner: Principal) -> Result<usize, String> {
    let consolidated = get_parallel_data(owner)?;
    let total_size = consolidated.len();

    BUFFER_MAPS.with(|maps| maps.borrow_mut().remove(&owner));
    BUFFERS.with(|buffers| {
        buffers.borrow_mut().insert(owner, consolidated);
    });

    Ok(total_size)
}

/// Get consolidated data from the owner's parallel chunks without moving them.
///
/// Combines chunks in order but leaves them in the parallel buffer.
///
/// # Errors
///
/// Returns an error if the owner's parallel buffer is empty.
pub fn get_parallel_data(owner: Principal) -> Result<Vec<u8>, String> {
    BUFFER_MAPS.with(|maps| {
        let maps = maps.borrow();
        let map = maps
            .get(&owner)
            .filter(|m| !m.is_empty())
            .ok_or_else(|| "No parallel chunks available".to_string())?;
        Ok(concat_in_order(map))
    })
}

/// Concatenate chunks in ascending `chunk_id` order.
fn concat_in_order(map: &HashMap<u32, Vec<u8>>) -> Vec<u8> {
    let mut pairs: Vec<(u32, &Vec<u8>)> = map.iter().map(|(id, c)| (*id, c)).collect();
    pairs.sort_unstable_by_key(|(id, _)| *id);

    let total: usize = pairs.iter().map(|(_, c)| c.len()).sum();
    let mut out = Vec::with_capacity(total);
    for (_, chunk) in pairs {
        out.extend_from_slice(chunk);
    }
    out
}

/// Clear all of the owner's parallel chunks.
pub fn clear_parallel_chunks(owner: Principal) {
    BUFFER_MAPS.with(|maps| {
        maps.borrow_mut().remove(&owner);
    });
}

/// Remove a specific chunk from the owner's parallel buffer.
///
/// # Returns
///
/// `true` if the chunk was present and removed.
pub fn remove_parallel_chunk(owner: Principal, chunk_id: u32) -> bool {
    BUFFER_MAPS.with(|maps| {
        let mut maps = maps.borrow_mut();
        let Some(map) = maps.get_mut(&owner) else {
            return false;
        };
        let removed = map.remove(&chunk_id).is_some();
        if map.is_empty() {
            maps.remove(&owner);
        }
        removed
    })
}

// ═══════════════════════════════════════════════════════════════
//  Storage Status and Monitoring
// ═══════════════════════════════════════════════════════════════

/// Get detailed status of the owner's buffers.
pub fn storage_status(owner: Principal) -> StorageStatus {
    let (parallel_chunk_count, parallel_buffer_size, parallel_chunk_ids) =
        BUFFER_MAPS.with(|maps| {
            maps.borrow().get(&owner).map_or((0, 0, Vec::new()), |map| {
                let mut ids: Vec<u32> = map.keys().copied().collect();
                ids.sort_unstable();
                (map.len(), map.values().map(Vec::len).sum(), ids)
            })
        });

    StorageStatus {
        buffer_size: buffer_size(owner),
        parallel_chunk_count,
        parallel_buffer_size,
        parallel_chunk_ids,
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
/// with optional storage integration. Every endpoint operates on the buffer
/// belonging to `ic_cdk::api::msg_caller()`, so concurrent uploads from
/// different principals are isolated from each other.
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
/// - `append_chunk(chunk: Vec<u8>) -> Result<usize, String>`
/// - `buffer_size() -> usize`
/// - `clear_buffer()`
///
/// **Parallel uploads:**
/// - `append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) -> Result<usize, String>`
/// - `parallel_chunks_complete(expected_count: u32) -> bool`
/// - `missing_chunks(expected_count: u32) -> Vec<u32>`
/// - `clear_parallel_chunks()`
/// - `parallel_chunk_count() -> usize`
///
/// **Storage (if registry provided; all guarded, since they expose arbitrary
/// keys in the registry):**
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
        $crate::generate_upload_endpoints!(guard = $guard);

        // Storage integration endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn save_buffer_to_storage(key: String) -> Result<String, String> {
            let owner = ic_cdk::api::msg_caller();
            let data = $crate::large_objects::get_buffer_data(owner);
            if data.is_empty() {
                return Err("No data in buffer".to_string());
            }

            let size = data.len();
            $registry.with(|r| {
                $crate::storage::save_bytes(r, &key, data);
            });

            #[cfg(feature = "telemetry")]
            $crate::telemetry::log_info(&format!("Saved {} bytes to key '{}'", size, key));

            Ok(format!("Saved {} bytes to key '{}'", size, key))
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn save_parallel_to_storage(key: String) -> Result<String, String> {
            let owner = ic_cdk::api::msg_caller();
            let data = $crate::large_objects::get_parallel_data(owner)?;
            let size = data.len();

            $registry.with(|r| {
                $crate::storage::save_bytes(r, &key, data);
            });

            $crate::large_objects::clear_parallel_chunks(owner);

            #[cfg(feature = "telemetry")]
            $crate::telemetry::log_info(&format!("Saved {} bytes to key '{}'", size, key));

            Ok(format!("Saved {} bytes to key '{}'", size, key))
        }

        #[ic_cdk::query(guard = $guard)]
        pub fn storage_key_exists(key: String) -> bool {
            $registry.with(|r| $crate::storage::exists(r, &key))
        }

        #[ic_cdk::query(guard = $guard)]
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
    };

    // Without storage registry (just upload endpoints)
    (guard = $guard:expr) => {
        // Sequential upload endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn append_chunk(chunk: Vec<u8>) -> Result<usize, String> {
            $crate::large_objects::append_chunk(ic_cdk::api::msg_caller(), chunk)
        }

        #[ic_cdk::query]
        pub fn buffer_size() -> usize {
            $crate::large_objects::buffer_size(ic_cdk::api::msg_caller())
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn clear_buffer() {
            $crate::large_objects::clear_buffer(ic_cdk::api::msg_caller());
        }

        // Parallel upload endpoints
        #[ic_cdk::update(guard = $guard)]
        pub fn append_parallel_chunk(chunk_id: u32, chunk: Vec<u8>) -> Result<usize, String> {
            $crate::large_objects::append_parallel_chunk(
                ic_cdk::api::msg_caller(),
                chunk_id,
                chunk,
            )
        }

        #[ic_cdk::query]
        pub fn parallel_chunks_complete(expected_count: u32) -> bool {
            $crate::large_objects::parallel_chunks_complete(
                ic_cdk::api::msg_caller(),
                expected_count,
            )
        }

        #[ic_cdk::query]
        pub fn missing_chunks(expected_count: u32) -> Vec<u32> {
            $crate::large_objects::missing_chunks(ic_cdk::api::msg_caller(), expected_count)
        }

        #[ic_cdk::update(guard = $guard)]
        pub fn clear_parallel_chunks() {
            $crate::large_objects::clear_parallel_chunks(ic_cdk::api::msg_caller());
        }

        #[ic_cdk::query]
        pub fn parallel_chunk_count() -> usize {
            $crate::large_objects::parallel_chunk_count(ic_cdk::api::msg_caller())
        }

        // Status endpoint
        #[ic_cdk::query]
        pub fn get_storage_status() -> String {
            $crate::large_objects::storage_status(ic_cdk::api::msg_caller()).to_string()
        }
    };

    // No guard (public endpoints - not recommended for production!)
    () => {
        fn __allow_all_uploads() -> Result<(), String> {
            Ok(())
        }
        $crate::generate_upload_endpoints!(guard = "__allow_all_uploads");
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner_a() -> Principal {
        Principal::anonymous()
    }

    fn owner_b() -> Principal {
        Principal::from_slice(&[1, 2, 3])
    }

    #[test]
    fn test_sequential_buffer() {
        let owner = owner_a();
        clear_buffer(owner);

        append_chunk(owner, vec![1, 2, 3]).unwrap();
        append_chunk(owner, vec![4, 5, 6]).unwrap();

        assert_eq!(buffer_size(owner), 6);

        let data = get_buffer_data(owner);
        assert_eq!(data, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(buffer_size(owner), 0);
    }

    #[test]
    fn test_parallel_chunks() {
        let owner = owner_a();
        clear_parallel_chunks(owner);

        append_parallel_chunk(owner, 2, vec![5, 6]).unwrap();
        append_parallel_chunk(owner, 0, vec![1, 2]).unwrap();
        append_parallel_chunk(owner, 1, vec![3, 4]).unwrap();

        assert_eq!(parallel_chunk_count(owner), 3);
        assert_eq!(parallel_buffer_size(owner), 6);
        assert!(parallel_chunks_complete(owner, 3));
        assert!(!parallel_chunks_complete(owner, 4));
        assert_eq!(parallel_chunk_ids(owner), vec![0, 1, 2]);

        assert_eq!(get_parallel_data(owner).unwrap(), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(parallel_chunk_count(owner), 3);

        assert_eq!(consolidate_parallel_chunks(owner).unwrap(), 6);
        assert_eq!(get_buffer_data(owner), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(parallel_chunk_count(owner), 0);

        // Consolidating with nothing buffered is an error and leaves state alone.
        assert!(consolidate_parallel_chunks(owner).is_err());
        assert_eq!(buffer_size(owner), 0);
    }

    #[test]
    fn test_owners_are_isolated() {
        let (a, b) = (owner_a(), owner_b());
        clear_buffer(a);
        clear_buffer(b);

        append_chunk(a, vec![1, 1]).unwrap();
        append_chunk(b, vec![2, 2, 2]).unwrap();
        append_chunk(a, vec![1]).unwrap();

        assert_eq!(get_buffer_data(a), vec![1, 1, 1]);
        assert_eq!(get_buffer_data(b), vec![2, 2, 2]);

        append_parallel_chunk(a, 0, vec![9]).unwrap();
        assert_eq!(parallel_chunk_count(b), 0);
        assert_eq!(missing_chunks(b, 2), vec![0, 1]);
        clear_parallel_chunks(a);
    }

    #[test]
    fn test_byte_cap_enforced() {
        let owner = owner_b();
        clear_buffer(owner);
        clear_parallel_chunks(owner);

        set_max_bytes_per_owner(Some(4));

        append_chunk(owner, vec![0; 3]).unwrap();
        assert!(append_chunk(owner, vec![0; 2]).is_err());
        // Parallel bytes count against the same cap.
        assert!(append_parallel_chunk(owner, 0, vec![0; 2]).is_err());
        append_parallel_chunk(owner, 0, vec![0; 1]).unwrap();
        // Replacing a chunk only counts the delta.
        append_parallel_chunk(owner, 0, vec![0; 1]).unwrap();

        set_max_bytes_per_owner(None);
        append_chunk(owner, vec![0; 100]).unwrap();

        set_max_bytes_per_owner(Some(DEFAULT_MAX_BYTES_PER_OWNER));
        clear_buffer(owner);
        clear_parallel_chunks(owner);
    }

    #[test]
    fn test_total_cap_spans_owners() {
        let (a, b) = (owner_a(), owner_b());
        clear_buffer(a);
        clear_buffer(b);
        clear_parallel_chunks(a);
        clear_parallel_chunks(b);

        set_max_bytes_per_owner(None);
        set_max_total_bytes(Some(5));

        append_chunk(a, vec![0; 3]).unwrap();
        // Owner B is under any per-owner cap but pushes the total past 5.
        assert!(append_chunk(b, vec![0; 3]).is_err());
        append_parallel_chunk(b, 0, vec![0; 2]).unwrap();
        assert_eq!(total_buffered_bytes_all_owners(), 5);
        // Replacing a parallel chunk with one of equal size is a zero delta.
        append_parallel_chunk(b, 0, vec![1; 2]).unwrap();
        // Replacing the sequential buffer counts only the delta too.
        load_to_buffer(a, vec![0; 3]).unwrap();
        assert!(load_to_buffer(a, vec![0; 4]).is_err());

        set_max_total_bytes(Some(DEFAULT_MAX_TOTAL_BYTES));
        set_max_bytes_per_owner(Some(DEFAULT_MAX_BYTES_PER_OWNER));
        clear_buffer(a);
        clear_buffer(b);
        clear_parallel_chunks(b);
    }

    #[test]
    fn test_empty_writes_do_not_leak_entries() {
        let owner = Principal::from_slice(&[7, 7]);
        append_chunk(owner, vec![]).unwrap();
        assert!(BUFFERS.with(|b| !b.borrow().contains_key(&owner)));

        append_parallel_chunk(owner, 0, vec![1]).unwrap();
        assert!(remove_parallel_chunk(owner, 0));
        assert!(!remove_parallel_chunk(owner, 0));
        assert!(BUFFER_MAPS.with(|m| !m.borrow().contains_key(&owner)));
    }
}
