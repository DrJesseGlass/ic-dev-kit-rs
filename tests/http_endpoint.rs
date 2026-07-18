//! Compile-time and roundtrip checks that the HTTP types work as real IC
//! endpoint types. The `#[ic_cdk::query]` attribute requires its argument and
//! return types to implement `CandidType`, so this file failing to compile
//! means the README quick-start is broken.

use ic_dev_kit_rs::http::{
    self, HttpError, HttpRequest, HttpResponse, StreamingCallback, StreamingCallbackHttpResponse,
    StreamingCallbackToken, StreamingStrategy,
};

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

#[ic_cdk::query]
fn http_request_streaming_callback(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    StreamingCallbackHttpResponse {
        body: token.key.into_bytes(),
        token: None,
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
    assert!(decoded.streaming_strategy.is_none());
}

/// The gateway reads `streaming_strategy` from the candid wire, so it must
/// survive an encode/decode roundtrip even though it is skipped in JSON.
#[test]
fn streaming_strategy_roundtrips_through_candid() {
    let token = StreamingCallbackToken {
        key: "/models/weights.bin".to_string(),
        content_encoding: "identity".to_string(),
        index: candid::Nat::from(1u64),
        sha256: None,
    };
    let response = http::json_response(200, "chunk-0".to_string()).with_streaming_strategy(
        StreamingStrategy::Callback {
            callback: StreamingCallback::new(
                candid::Principal::from_text("aaaaa-aa").unwrap(),
                "http_request_streaming_callback".to_string(),
            ),
            token: token.clone(),
        },
    );

    let bytes = candid::encode_one(&response).unwrap();
    let decoded: HttpResponse = candid::decode_one(&bytes).unwrap();

    let StreamingStrategy::Callback {
        callback,
        token: decoded_token,
    } = decoded.streaming_strategy.expect("strategy lost on the candid wire");
    assert_eq!(callback.0.method, "http_request_streaming_callback");
    assert_eq!(decoded_token, token);

    // JSON output must still work and simply omit the strategy.
    let json = serde_json::to_string(&response).unwrap();
    assert!(!json.contains("streaming_strategy"));
}
