# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

Initial release.

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