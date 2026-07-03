# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-07-03

Correctness and hardening release: HTTP types now work as IC endpoint types,
upload buffers are isolated per caller and size-capped, the auth/telemetry
modules are decoupled, and CI now checks the `wasm32-unknown-unknown` target.
Contains breaking API changes in `auth` and `large_objects` — see below.

### Fixed

- **http** - `HttpRequest` and `HttpResponse` now derive `CandidType`. Previously
  they only derived serde traits, so using them as `#[ic_cdk::query]` argument or
  return types (as documented in the README) failed to compile. An integration
  test (`tests/http_endpoint.rs`) and a non-ignored doctest now exercise the
  types through a real endpoint to keep this from regressing.

### Changed (breaking)

- **large_objects** - Buffers are now keyed by an `owner: Principal` (pass
  `ic_cdk::api::msg_caller()` from endpoints). Previously all callers shared a
  single global buffer, so concurrent uploads from different principals could
  interleave and corrupt each other's data. All functions take `owner` as their
  first argument; `generate_upload_endpoints!` passes the caller automatically.
- **large_objects** - Total buffered bytes per owner are capped
  (`DEFAULT_MAX_BYTES_PER_OWNER` = 2 GiB); `append_chunk`, `append_parallel_chunk`,
  and `load_to_buffer` now return `Result` and reject writes that would exceed
  the cap. Configure with `set_max_bytes_per_owner` (`None` disables).
- **auth** - Removed the `AuthStorage` type and the internal storage/cache split;
  both were in-memory `HashSet`s holding duplicate copies of the same data.
  `Auth::new()` takes no arguments; `Auth::with_principals(...)` replaces
  `AuthStorage::with_initial_principal`. `save_to_storage`/`load_from_storage`
  are gone — upgrade persistence remains via `save_to_bytes`/`init_from_saved`.
- **auth** - Guard and management functions no longer trap when auth was never
  initialized; they return an error instead. Removed unused
  `AuthError::StorageError`/`SerializationError` variants.
- **telemetry** - `export_telemetry_endpoints!` is now self-contained: the
  monitoring-admin endpoints are guarded by the new
  `telemetry::is_monitoring_admin` (controllers always allowed, plus `auth`
  admins when initialized) instead of silently requiring
  `export_auth_endpoints!` to have been invoked first in the same scope.
- **telemetry** - Monitoring-principal state lazily initializes on first use
  instead of trapping when `telemetry::init()` was not called.

### Added

- `export_telemetry_endpoints!(admin_guard = "my_guard")` - optional macro arm
  to supply a custom guard for the monitoring administration endpoints
  (defaults to `telemetry::is_monitoring_admin`).
- GitHub Actions CI: native tests plus `cargo check` for
  `wasm32-unknown-unknown` (the target consumers actually build for), with and
  without the ML features.

## [0.1.0] - 2026-06-29

Initial release. Built against `ic-cdk` 0.20, `candid` 0.10, `ic-stable-structures`
0.7, and `candle` 0.11. The optional `telemetry` feature uses the
`canistergeek_ic_rust` fork (tag `v0.6.0`) migrated to `ic-cdk` 0.20; all git
dependencies are pinned to immutable refs / published versions for reproducibility.

### Added

- **auth** - Principal-based authorization with guard functions
- **http** - IC-compatible HTTP request/response types, routing, JSON utilities
- **large_objects** - Chunked upload system for large files (sequential and parallel)
- **intercanister** - Inter-canister call wrappers with automatic logging
- **storage** - Type-safe stable storage utilities (feature-gated)
- **telemetry** - Canistergeek monitoring and logging integration (feature-gated)
- **candle** - Generic ML model infrastructure (feature-gated)
- **text_generation** - LLM text generation traits and utilities (feature-gated)
- **model_server** - Ready-to-use LLM inference server (feature-gated)

### Macros

- `export_auth_endpoints!` - Generate auth management IC endpoints
- `export_telemetry_endpoints!` - Generate Canistergeek-compatible endpoints
- `generate_upload_endpoints!` - Generate large object upload endpoints
- `generate_model_endpoints!` - Generate ML model inference endpoints