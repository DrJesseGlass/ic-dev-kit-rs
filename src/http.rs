//! HTTP handling module for Internet Computer canisters.
//!
//! Provides IC-compatible HTTP request/response types, routing, and utilities.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use ic_dev_kit_rs::http::{self, HttpRequest, HttpResponse};
//!
//! #[ic_cdk::query]
//! fn http_request(req: HttpRequest) -> HttpResponse {
//!     let path = http::extract_path(&req.url);
//!
//!     match (req.method.as_str(), path) {
//!         ("GET", "/api/status") => {
//!             http::success_response(&serde_json::json!({"status": "ok"})).unwrap()
//!         }
//!         _ => http::HttpError::NotFound.to_response(),
//!     }
//! }
//! ```
//!
//! # Router Example
//!
//! ```rust,ignore
//! use ic_dev_kit_rs::http::{Router, HttpMethod, HttpRequest, HttpResponse, HttpResult};
//!
//! fn status_handler(_req: HttpRequest) -> HttpResult<HttpResponse> {
//!     http::success_response(&"ok")
//! }
//!
//! let mut router = Router::new();
//! router.get("/api/status", status_handler);
//!
//! let response = router.handle(request);
//! ```

use candid::CandidType;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
//  Error Types
// ═══════════════════════════════════════════════════════════════

/// HTTP errors with automatic status code mapping.
///
/// Each variant maps to an appropriate HTTP status code and can be
/// converted directly to an [`HttpResponse`].
///
/// # Example
///
/// ```rust,ignore
/// fn my_handler(req: HttpRequest) -> HttpResult<HttpResponse> {
///     if !valid {
///         return Err(HttpError::bad_request("Invalid input"));
///     }
///     // ...
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// 405 Method Not Allowed
    #[error("Method not allowed")]
    MethodNotAllowed,
    /// 404 Not Found
    #[error("Endpoint not found")]
    NotFound,
    /// 400 Bad Request - invalid request format
    #[error("Invalid request format: {0}")]
    InvalidRequest(String),
    /// 401 Unauthorized
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    /// 500 Internal Server Error
    #[error("Internal server error: {0}")]
    InternalError(String),
    /// 500 Internal Server Error - serialization failure
    #[error("Serialization error: {0}")]
    SerializationError(String),
    /// 400 Bad Request
    #[error("Bad request: {0}")]
    BadRequest(String),
    /// 409 Conflict
    #[error("Conflict: {0}")]
    Conflict(String),
    /// 422 Unprocessable Entity
    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),
    /// 403 Forbidden
    #[error("Forbidden: {0}")]
    Forbidden(String),
    /// Custom status code
    #[error("HTTP {status}: {message}")]
    Status {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },
}

impl HttpError {
    /// Get the HTTP status code for this error.
    pub fn status_code(&self) -> u16 {
        match self {
            HttpError::MethodNotAllowed => 405,
            HttpError::NotFound => 404,
            HttpError::InvalidRequest(_) => 400,
            HttpError::Unauthorized(_) => 401,
            HttpError::InternalError(_) => 500,
            HttpError::SerializationError(_) => 500,
            HttpError::BadRequest(_) => 400,
            HttpError::Conflict(_) => 409,
            HttpError::UnprocessableEntity(_) => 422,
            HttpError::Forbidden(_) => 403,
            HttpError::Status { status, .. } => *status,
        }
    }

    /// Convert this error to an HTTP response.
    pub fn to_response(&self) -> HttpResponse {
        error_response(self.status_code(), &self.to_string())
    }

    /// Create a 400 Bad Request error.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        HttpError::BadRequest(msg.into())
    }

    /// Create a 404 Not Found error with custom message.
    pub fn not_found(msg: impl Into<String>) -> Self {
        HttpError::Status {
            status: 404,
            message: msg.into(),
        }
    }

    /// Create a 401 Unauthorized error.
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        HttpError::Unauthorized(msg.into())
    }

    /// Create a 409 Conflict error.
    pub fn conflict(msg: impl Into<String>) -> Self {
        HttpError::Conflict(msg.into())
    }

    /// Create a 422 Unprocessable Entity error.
    pub fn unprocessable_entity(msg: impl Into<String>) -> Self {
        HttpError::UnprocessableEntity(msg.into())
    }

    /// Create a 403 Forbidden error.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        HttpError::Forbidden(msg.into())
    }

    /// Create a 500 Internal Server Error.
    pub fn internal_error(msg: impl Into<String>) -> Self {
        HttpError::InternalError(msg.into())
    }

    /// Create an error with a custom status code.
    pub fn custom_status(status: u16, msg: impl Into<String>) -> Self {
        HttpError::Status {
            status,
            message: msg.into(),
        }
    }
}

/// Result type for HTTP operations.
pub type HttpResult<T> = Result<T, HttpError>;

// ═══════════════════════════════════════════════════════════════
//  HTTP Types
// ═══════════════════════════════════════════════════════════════

/// HTTP request structure (IC-compatible).
///
/// This matches the format expected by the IC HTTP gateway.
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Request URL including path and query string
    pub url: String,
    /// Request headers as key-value pairs
    pub headers: Vec<(String, String)>,
    /// Request body as raw bytes
    pub body: Vec<u8>,
}

/// HTTP response structure (IC-compatible).
///
/// This matches the format expected by the IC HTTP gateway.
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct HttpResponse {
    /// HTTP status code (200, 404, etc.)
    pub status_code: u16,
    /// Response headers as key-value pairs
    pub headers: Vec<(String, String)>,
    /// Response body as raw bytes
    pub body: Vec<u8>,
    /// Whether to upgrade to update call (for certified responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<bool>,
    /// Strategy for streaming the rest of the body via callback.
    ///
    /// Candid function references have no serde `Serialize` impl, so this
    /// field is omitted from JSON output; it is still present on the candid
    /// wire, which is what the HTTP gateway reads.
    #[serde(skip_serializing, default)]
    pub streaming_strategy: Option<StreamingStrategy>,
}

impl HttpResponse {
    /// Attach a streaming strategy to this response (builder-style).
    pub fn with_streaming_strategy(mut self, strategy: StreamingStrategy) -> Self {
        self.streaming_strategy = Some(strategy);
        self
    }
}

// ═══════════════════════════════════════════════════════════════
//  Streaming (IC HTTP gateway callback protocol)
// ═══════════════════════════════════════════════════════════════

// Reference to the query method the HTTP gateway calls to fetch the next
// body chunk: `(StreamingCallbackToken) -> (StreamingCallbackHttpResponse) query`.
candid::define_function!(
    pub StreamingCallback : (StreamingCallbackToken) -> (StreamingCallbackHttpResponse) query
);

/// Token passed back to the streaming callback to identify the next chunk.
///
/// The gateway treats this value as opaque and returns it verbatim to the
/// callback. The field layout follows the certified asset canister convention.
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct StreamingCallbackToken {
    /// Identifies the resource being streamed (e.g. a path or object key).
    pub key: String,
    /// Content encoding of the streamed body (e.g. "identity", "gzip").
    pub content_encoding: String,
    /// Zero-based index of the next chunk to return.
    pub index: candid::Nat,
    /// Optional SHA-256 of the full body, for certification flows.
    pub sha256: Option<Vec<u8>>,
}

/// Strategy for streaming a response body larger than one message.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum StreamingStrategy {
    /// The gateway repeatedly calls `callback` with the current token until
    /// the callback returns a response with no token.
    Callback {
        /// Query method to call for each subsequent chunk.
        callback: StreamingCallback,
        /// Token identifying the first chunk to fetch.
        token: StreamingCallbackToken,
    },
}

/// Response returned by a streaming callback: one chunk plus the token for
/// the next one (or `None` when the body is complete).
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct StreamingCallbackHttpResponse {
    /// This chunk of the response body.
    pub body: Vec<u8>,
    /// Token for the next chunk, or `None` if this was the last chunk.
    pub token: Option<StreamingCallbackToken>,
}

/// HTTP method enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    OPTIONS,
    HEAD,
}

impl HttpMethod {
    /// Parse an HTTP method from a string (case-insensitive).
    pub fn from_str(method: &str) -> Option<Self> {
        match method.to_uppercase().as_str() {
            "GET" => Some(HttpMethod::GET),
            "POST" => Some(HttpMethod::POST),
            "PUT" => Some(HttpMethod::PUT),
            "DELETE" => Some(HttpMethod::DELETE),
            "PATCH" => Some(HttpMethod::PATCH),
            "OPTIONS" => Some(HttpMethod::OPTIONS),
            "HEAD" => Some(HttpMethod::HEAD),
            _ => None,
        }
    }

    /// Get the method as a static string.
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::OPTIONS => "OPTIONS",
            HttpMethod::HEAD => "HEAD",
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Response Builders
// ═══════════════════════════════════════════════════════════════

/// Create a JSON response with the given status code.
///
/// Automatically sets `Content-Type: application/json` and CORS headers.
pub fn json_response(status_code: u16, body: String) -> HttpResponse {
    HttpResponse {
        status_code,
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
        ],
        body: body.into_bytes(),
        upgrade: None,
        streaming_strategy: None,
    }
}

/// Create an error response with JSON body.
///
/// Response format: `{"error": "<message>"}`
pub fn error_response(status_code: u16, error: &str) -> HttpResponse {
    json_response(
        status_code,
        format!(r#"{{"error":"{}"}}"#, escape_json(error)),
    )
}

/// Create a success response with JSON-serialized data.
///
/// # Errors
///
/// Returns [`HttpError::SerializationError`] if serialization fails.
pub fn success_response<T: Serialize>(data: &T) -> HttpResult<HttpResponse> {
    let json = serde_json::to_string(data)
        .map_err(|e| HttpError::SerializationError(format!("JSON serialization error: {}", e)))?;
    Ok(json_response(200, json))
}

/// Create a response indicating the request should be upgraded to an update call.
///
/// Used for certified queries that need to modify state.
pub fn upgrade_response() -> HttpResponse {
    HttpResponse {
        status_code: 204,
        headers: vec![],
        body: vec![],
        upgrade: Some(true),
        streaming_strategy: None,
    }
}

/// Create a CORS preflight response.
///
/// Responds to OPTIONS requests with appropriate CORS headers.
pub fn cors_preflight_response() -> HttpResponse {
    HttpResponse {
        status_code: 204,
        headers: vec![
            ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
            (
                "Access-Control-Allow-Methods".to_string(),
                "GET, POST, PUT, DELETE, PATCH, OPTIONS".to_string(),
            ),
            (
                "Access-Control-Allow-Headers".to_string(),
                "Content-Type, Authorization".to_string(),
            ),
        ],
        body: vec![],
        upgrade: None,
        streaming_strategy: None,
    }
}

/// Escape special characters in a JSON string.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ═══════════════════════════════════════════════════════════════
//  JSON Utilities
// ═══════════════════════════════════════════════════════════════

/// Parse JSON from request body bytes.
///
/// # Errors
///
/// Returns [`HttpError::InvalidRequest`] if the body is not valid UTF-8 or JSON.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Deserialize)]
/// struct MyData { name: String }
///
/// let data: MyData = http::parse_json(&req.body)?;
/// ```
pub fn parse_json<T>(body: &[u8]) -> HttpResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let body_str = String::from_utf8(body.to_vec())
        .map_err(|_| HttpError::InvalidRequest("Invalid UTF-8 in request body".to_string()))?;

    serde_json::from_str::<T>(&body_str)
        .map_err(|e| HttpError::InvalidRequest(format!("JSON parse error: {}", e)))
}

/// Serialize data to a JSON string.
///
/// # Errors
///
/// Returns [`HttpError::SerializationError`] if serialization fails.
pub fn to_json<T>(data: &T) -> HttpResult<String>
where
    T: Serialize,
{
    serde_json::to_string(data)
        .map_err(|e| HttpError::SerializationError(format!("JSON serialization error: {}", e)))
}

/// Serialize data to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns [`HttpError::SerializationError`] if serialization fails.
pub fn to_json_pretty<T>(data: &T) -> HttpResult<String>
where
    T: Serialize,
{
    serde_json::to_string_pretty(data)
        .map_err(|e| HttpError::SerializationError(format!("JSON serialization error: {}", e)))
}

// ═══════════════════════════════════════════════════════════════
//  Path Utilities
// ═══════════════════════════════════════════════════════════════

/// Extract the path from a URL (removes query string).
///
/// # Example
///
/// ```rust,ignore
/// assert_eq!(extract_path("/api/users?page=1"), "/api/users");
/// ```
pub fn extract_path(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Extract query parameters from a URL.
///
/// # Example
///
/// ```rust,ignore
/// let params = extract_query_params("/api/users?page=1&limit=10");
/// assert_eq!(params.get("page"), Some(&"1".to_string()));
/// ```
pub fn extract_query_params(url: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if let Some(query) = url.split('?').nth(1) {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }
    }

    params
}

/// Split a path into parts, filtering empty segments.
fn split_path_parts(value: &str) -> Vec<&str> {
    value.split('/').filter(|part| !part.is_empty()).collect()
}

/// Check if a path matches a pattern.
///
/// Supports:
/// - Exact matching: `/api/users`
/// - Wildcards: `/api/*` matches any single segment
/// - Parameters: `/api/users/:id` matches `/api/users/123`
///
/// # Example
///
/// ```rust,ignore
/// assert!(matches_pattern("/api/users/123", "/api/users/:id"));
/// assert!(matches_pattern("/api/v1/data", "/api/*"));
/// ```
pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    fn matches_recursive(path: &[&str], pattern: &[&str]) -> bool {
        if pattern.is_empty() {
            return path.is_empty();
        }

        match pattern[0] {
            "*" => {
                // "*" matches zero or more path segments
                (0..=path.len()).any(|skip| matches_recursive(&path[skip..], &pattern[1..]))
            }
            param if param.starts_with(':') => {
                !path.is_empty() && matches_recursive(&path[1..], &pattern[1..])
            }
            literal => {
                !path.is_empty()
                    && path[0] == literal
                    && matches_recursive(&path[1..], &pattern[1..])
            }
        }
    }

    let path_parts = split_path_parts(path);
    let pattern_parts = split_path_parts(pattern);

    matches_recursive(&path_parts, &pattern_parts)
}

/// Extract path parameters from a pattern match.
///
/// # Example
///
/// ```rust,ignore
/// let params = extract_params("/api/users/123", "/api/users/:id");
/// assert_eq!(params.get("id"), Some(&"123".to_string()));
/// ```
pub fn extract_params(path: &str, pattern: &str) -> HashMap<String, String> {
    fn match_with_params(
        path: &[&str],
        pattern: &[&str],
        params: &mut HashMap<String, String>,
    ) -> bool {
        if pattern.is_empty() {
            return path.is_empty();
        }

        match pattern[0] {
            "*" => {
                for skip in 0..=path.len() {
                    let mut snapshot = params.clone();
                    if match_with_params(&path[skip..], &pattern[1..], &mut snapshot) {
                        *params = snapshot;
                        return true;
                    }
                }
                false
            }
            param if param.starts_with(':') => {
                if path.is_empty() {
                    return false;
                }

                params.insert(param[1..].to_string(), path[0].to_string());
                match_with_params(&path[1..], &pattern[1..], params)
            }
            literal => {
                !path.is_empty()
                    && path[0] == literal
                    && match_with_params(&path[1..], &pattern[1..], params)
            }
        }
    }

    let mut params = HashMap::new();
    let path_parts = split_path_parts(path);
    let pattern_parts = split_path_parts(pattern);

    if match_with_params(&path_parts, &pattern_parts, &mut params) {
        params
    } else {
        HashMap::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  Header Utilities
// ═══════════════════════════════════════════════════════════════

/// Get a header value by name (case-insensitive).
///
/// # Example
///
/// ```rust,ignore
/// if let Some(content_type) = get_header(&req.headers, "content-type") {
///     // ...
/// }
/// ```
pub fn get_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == name.to_lowercase())
        .map(|(_, v)| v.as_str())
}

/// Extract bearer token from Authorization header.
///
/// # Example
///
/// ```rust,ignore
/// if let Some(token) = extract_bearer_token(&req.headers) {
///     // Validate token...
/// }
/// ```
pub fn extract_bearer_token(headers: &[(String, String)]) -> Option<String> {
    get_header(headers, "Authorization").and_then(|value| {
        if value.starts_with("Bearer ") {
            Some(value[7..].to_string())
        } else {
            None
        }
    })
}

// ═══════════════════════════════════════════════════════════════
//  HTTP Status Codes
// ═══════════════════════════════════════════════════════════════

/// Common HTTP status code constants.
pub mod status {
    /// 200 OK
    pub const OK: u16 = 200;
    /// 201 Created
    pub const CREATED: u16 = 201;
    /// 202 Accepted
    pub const ACCEPTED: u16 = 202;
    /// 204 No Content
    pub const NO_CONTENT: u16 = 204;
    /// 400 Bad Request
    pub const BAD_REQUEST: u16 = 400;
    /// 401 Unauthorized
    pub const UNAUTHORIZED: u16 = 401;
    /// 403 Forbidden
    pub const FORBIDDEN: u16 = 403;
    /// 404 Not Found
    pub const NOT_FOUND: u16 = 404;
    /// 405 Method Not Allowed
    pub const METHOD_NOT_ALLOWED: u16 = 405;
    /// 409 Conflict
    pub const CONFLICT: u16 = 409;
    /// 422 Unprocessable Entity
    pub const UNPROCESSABLE_ENTITY: u16 = 422;
    /// 500 Internal Server Error
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
    /// 502 Bad Gateway
    pub const BAD_GATEWAY: u16 = 502;
    /// 503 Service Unavailable
    pub const SERVICE_UNAVAILABLE: u16 = 503;
}

// ═══════════════════════════════════════════════════════════════
//  Result Extension Trait
// ═══════════════════════════════════════════════════════════════

/// Extension trait to convert results to HTTP responses.
pub trait IntoHttpResponse {
    /// Convert to an HTTP response.
    fn into_http_response(self) -> HttpResult<HttpResponse>;
}

impl<T: Serialize> IntoHttpResponse for Result<T, HttpError> {
    fn into_http_response(self) -> HttpResult<HttpResponse> {
        match self {
            Ok(data) => success_response(&data),
            Err(e) => Ok(e.to_response()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Simple Router Implementation
// ═══════════════════════════════════════════════════════════════

/// Handler function type for router.
pub type HandlerFn = fn(HttpRequest) -> HttpResult<HttpResponse>;

/// Simple HTTP router with pattern matching.
///
/// # Example
///
/// ```rust,ignore
/// let mut router = Router::new();
/// router.get("/api/status", status_handler);
/// router.post("/api/users", create_user_handler);
///
/// let response = router.handle(request);
/// ```
pub struct Router {
    routes: HashMap<(HttpMethod, String), HandlerFn>,
}

impl Router {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Add a route with a specific method.
    pub fn add_route(&mut self, method: HttpMethod, path: impl Into<String>, handler: HandlerFn) {
        self.routes.insert((method, path.into()), handler);
    }

    /// Add a GET route.
    pub fn get(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::GET, path, handler);
    }

    /// Add a POST route.
    pub fn post(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::POST, path, handler);
    }

    /// Add a PUT route.
    pub fn put(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::PUT, path, handler);
    }

    /// Add a DELETE route.
    pub fn delete(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::DELETE, path, handler);
    }

    /// Handle an incoming request.
    ///
    /// Automatically handles CORS preflight (OPTIONS) requests.
    pub fn handle(&self, request: HttpRequest) -> HttpResponse {
        // Handle CORS preflight
        if request.method.to_uppercase() == "OPTIONS" {
            return cors_preflight_response();
        }

        let method = match HttpMethod::from_str(&request.method) {
            Some(m) => m,
            None => return HttpError::MethodNotAllowed.to_response(),
        };

        let path = extract_path(&request.url);

        // Try exact match first
        if let Some(handler) = self.routes.get(&(method.clone(), path.to_string())) {
            return handler(request).unwrap_or_else(|e| e.to_response());
        }

        // Try pattern matching
        for ((route_method, route_path), handler) in &self.routes {
            if route_method == &method && matches_pattern(path, route_path) {
                return handler(request).unwrap_or_else(|e| e.to_response());
            }
        }

        HttpError::NotFound.to_response()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_error_status_codes() {
        assert_eq!(HttpError::BadRequest("test".to_string()).status_code(), 400);
        assert_eq!(HttpError::Conflict("test".to_string()).status_code(), 409);
        assert_eq!(
            HttpError::UnprocessableEntity("test".to_string()).status_code(),
            422
        );
        assert_eq!(
            HttpError::custom_status(418, "I'm a teapot").status_code(),
            418
        );
    }

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(HttpMethod::from_str("GET"), Some(HttpMethod::GET));
        assert_eq!(HttpMethod::from_str("post"), Some(HttpMethod::POST));
        assert_eq!(HttpMethod::from_str("INVALID"), None);
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("/api/test"), "/api/test");
        assert_eq!(extract_path("/api/test?param=value"), "/api/test");
        assert_eq!(
            extract_path("/api/test?param1=value1&param2=value2"),
            "/api/test"
        );
    }

    #[test]
    fn test_extract_query_params() {
        let params = extract_query_params("/api/test?foo=bar&baz=qux");
        assert_eq!(params.get("foo"), Some(&"bar".to_string()));
        assert_eq!(params.get("baz"), Some(&"qux".to_string()));
    }

    #[test]
    fn test_path_matching() {
        assert!(matches_pattern("/api/test", "/api/test"));
        assert!(matches_pattern("/api/test", "/api/*"));
        assert!(matches_pattern("/api/v1/users", "*/users"));
        assert!(matches_pattern("/api/v1/users/123", "*/users/*"));
        assert!(!matches_pattern("/api/test", "/api/other"));
    }

    #[test]
    fn test_extract_params() {
        let params = extract_params("/api/users/123", "/api/users/:id");
        assert_eq!(params.get("id"), Some(&"123".to_string()));

        let params = extract_params(
            "/api/users/123/posts/456",
            "/api/users/:userId/posts/:postId",
        );
        assert_eq!(params.get("userId"), Some(&"123".to_string()));
        assert_eq!(params.get("postId"), Some(&"456".to_string()));

        let params = extract_params("/api/v1/users/123", "*/users/:id");
        assert_eq!(params.get("id"), Some(&"123".to_string()));
    }

    #[test]
    fn test_json_utilities() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        let json = to_json(&data).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("42"));

        let parsed: TestData = parse_json(json.as_bytes()).unwrap();
        assert_eq!(parsed, data);
    }

    #[test]
    fn test_get_header() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), "Bearer token123".to_string()),
        ];

        assert_eq!(
            get_header(&headers, "content-type"),
            Some("application/json")
        );
        assert_eq!(
            get_header(&headers, "Authorization"),
            Some("Bearer token123")
        );
        assert_eq!(get_header(&headers, "Missing"), None);
    }

    #[test]
    fn test_extract_bearer_token() {
        let headers = vec![("Authorization".to_string(), "Bearer token123".to_string())];

        assert_eq!(extract_bearer_token(&headers), Some("token123".to_string()));

        let headers = vec![("Authorization".to_string(), "Basic xyz".to_string())];
        assert_eq!(extract_bearer_token(&headers), None);
    }
}