//! Compile-time and roundtrip checks that the HTTP types work as real IC
//! endpoint types. The `#[ic_cdk::query]` attribute requires its argument and
//! return types to implement `CandidType`, so this file failing to compile
//! means the README quick-start is broken.

use ic_dev_kit_rs::http::{self, HttpError, HttpRequest, HttpResponse};

#[ic_cdk::query]
fn http_request(req: HttpRequest) -> HttpResponse {
    let path = http::extract_path(&req.url);

    match (req.method.as_str(), path) {
        ("GET", "/api/status") => {
            http::success_response(&serde_json::json!({"status": "ok"})).unwrap()
        }
        _ => HttpError::NotFound.to_response(),
    }
}

#[test]
fn http_types_roundtrip_through_candid() {
    let request = HttpRequest {
        method: "GET".to_string(),
        url: "/api/status?verbose=1".to_string(),
        headers: vec![("Accept".to_string(), "application/json".to_string())],
        body: vec![],
    };

    let bytes = candid::encode_one(&request).unwrap();
    let decoded: HttpRequest = candid::decode_one(&bytes).unwrap();
    assert_eq!(decoded.method, "GET");
    assert_eq!(decoded.url, "/api/status?verbose=1");

    let response = http::json_response(200, r#"{"status":"ok"}"#.to_string());
    let bytes = candid::encode_one(&response).unwrap();
    let decoded: HttpResponse = candid::decode_one(&bytes).unwrap();
    assert_eq!(decoded.status_code, 200);
    assert_eq!(decoded.body, br#"{"status":"ok"}"#.to_vec());
}
