// HTTP handling module for Internet Computer canisters
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
//  Error Types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("Method not allowed")]
    MethodNotAllowed,
    #[error("Endpoint not found")]
    NotFound,
    #[error("Invalid request format: {0}")]
    InvalidRequest(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("HTTP {status}: {message}")]
    Status { status: u16, message: String },
}

impl HttpError {
    /// Get the HTTP status code for this error
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

    /// Convert to HTTP response automatically
    pub fn to_response(&self) -> HttpResponse {
        error_response(self.status_code(), &self.to_string())
    }

    // Convenience constructors
    pub fn bad_request(msg: impl Into<String>) -> Self {
        HttpError::BadRequest(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        HttpError::Status {
            status: 404,
            message: msg.into(),
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        HttpError::Unauthorized(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        HttpError::Conflict(msg.into())
    }

    pub fn unprocessable_entity(msg: impl Into<String>) -> Self {
        HttpError::UnprocessableEntity(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        HttpError::Forbidden(msg.into())
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        HttpError::InternalError(msg.into())
    }

    pub fn custom_status(status: u16, msg: impl Into<String>) -> Self {
        HttpError::Status {
            status,
            message: msg.into(),
        }
    }
}

pub type HttpResult<T> = Result<T, HttpError>;

// ═══════════════════════════════════════════════════════════════
//  HTTP Types
// ═══════════════════════════════════════════════════════════════

/// HTTP request structure (IC-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// HTTP response structure (IC-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<bool>,
}

/// HTTP method enumeration
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

pub fn json_response(status_code: u16, body: String) -> HttpResponse {
    HttpResponse {
        status_code,
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
        ],
        body: body.into_bytes(),
        upgrade: None,
    }
}

pub fn error_response(status_code: u16, error: &str) -> HttpResponse {
    json_response(
        status_code,
        format!(r#"{{"error":"{}"}}"#, escape_json(error)),
    )
}

pub fn success_response<T: Serialize>(data: &T) -> HttpResult<HttpResponse> {
    let json = serde_json::to_string(data)
        .map_err(|e| HttpError::SerializationError(format!("JSON serialization error: {}", e)))?;
    Ok(json_response(200, json))
}

pub fn upgrade_response() -> HttpResponse {
    HttpResponse {
        status_code: 204,
        headers: vec![],
        body: vec![],
        upgrade: Some(true),
    }
}

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
    }
}

// Helper to escape JSON strings
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

pub fn parse_json<T>(body: &[u8]) -> HttpResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let body_str = String::from_utf8(body.to_vec())
        .map_err(|_| HttpError::InvalidRequest("Invalid UTF-8 in request body".to_string()))?;

    serde_json::from_str::<T>(&body_str)
        .map_err(|e| HttpError::InvalidRequest(format!("JSON parse error: {}", e)))
}

pub fn to_json<T>(data: &T) -> HttpResult<String>
where
    T: Serialize,
{
    serde_json::to_string(data)
        .map_err(|e| HttpError::SerializationError(format!("JSON serialization error: {}", e)))
}

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

/// Extract the path from a URL (removes query string)
pub fn extract_path(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Extract query parameters from a URL
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

/// Check if a path matches a pattern (with wildcard support)
pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    let path_parts: Vec<&str> = path.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if path_parts.len() != pattern_parts.len() {
        return false;
    }

    for (path_part, pattern_part) in path_parts.iter().zip(pattern_parts.iter()) {
        if pattern_part == &"*" {
            continue;
        }
        if pattern_part.starts_with(':') {
            continue;
        }
        if path_part != pattern_part {
            return false;
        }
    }

    true
}

/// Extract path parameters from a pattern match
pub fn extract_params(path: &str, pattern: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let path_parts: Vec<&str> = path.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if path_parts.len() != pattern_parts.len() {
        return params;
    }

    for (path_part, pattern_part) in path_parts.iter().zip(pattern_parts.iter()) {
        if pattern_part.starts_with(':') {
            let param_name = &pattern_part[1..];
            params.insert(param_name.to_string(), path_part.to_string());
        } else if path_part != pattern_part {
            return HashMap::new();
        }
    }

    params
}

// ═══════════════════════════════════════════════════════════════
//  Header Utilities
// ═══════════════════════════════════════════════════════════════

/// Get header value by name (case-insensitive)
pub fn get_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == name.to_lowercase())
        .map(|(_, v)| v.as_str())
}

/// Extract bearer token from Authorization header
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

pub mod status {
    pub const OK: u16 = 200;
    pub const CREATED: u16 = 201;
    pub const ACCEPTED: u16 = 202;
    pub const NO_CONTENT: u16 = 204;
    pub const BAD_REQUEST: u16 = 400;
    pub const UNAUTHORIZED: u16 = 401;
    pub const FORBIDDEN: u16 = 403;
    pub const NOT_FOUND: u16 = 404;
    pub const METHOD_NOT_ALLOWED: u16 = 405;
    pub const CONFLICT: u16 = 409;
    pub const UNPROCESSABLE_ENTITY: u16 = 422;
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
    pub const BAD_GATEWAY: u16 = 502;
    pub const SERVICE_UNAVAILABLE: u16 = 503;
}

// ═══════════════════════════════════════════════════════════════
//  Result Extension Trait
// ═══════════════════════════════════════════════════════════════

/// Extension trait to convert results to HTTP responses
pub trait IntoHttpResponse {
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

pub type HandlerFn = fn(HttpRequest) -> HttpResult<HttpResponse>;

pub struct Router {
    routes: HashMap<(HttpMethod, String), HandlerFn>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn add_route(&mut self, method: HttpMethod, path: impl Into<String>, handler: HandlerFn) {
        self.routes.insert((method, path.into()), handler);
    }

    pub fn get(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::GET, path, handler);
    }

    pub fn post(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::POST, path, handler);
    }

    pub fn put(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::PUT, path, handler);
    }

    pub fn delete(&mut self, path: impl Into<String>, handler: HandlerFn) {
        self.add_route(HttpMethod::DELETE, path, handler);
    }

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

        assert_eq!(get_header(&headers, "content-type"), Some("application/json"));
        assert_eq!(get_header(&headers, "Authorization"), Some("Bearer token123"));
        assert_eq!(get_header(&headers, "Missing"), None);
    }

    #[test]
    fn test_extract_bearer_token() {
        let headers = vec![(
            "Authorization".to_string(),
            "Bearer token123".to_string(),
        )];

        assert_eq!(
            extract_bearer_token(&headers),
            Some("token123".to_string())
        );

        let headers = vec![("Authorization".to_string(), "Basic xyz".to_string())];
        assert_eq!(extract_bearer_token(&headers), None);
    }
}