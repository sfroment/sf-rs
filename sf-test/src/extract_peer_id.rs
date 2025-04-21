use axum::{
    extract::FromRequestParts,
    http::{HeaderName, HeaderValue, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sf_logging::{debug, warn};

pub type PeerID = String;

pub struct ExtractPeerID(pub PeerID);

const PEER_ID_HEADER_STR: &str = "x-sf-peer-id";
const PEER_ID_HEADER: HeaderName = HeaderName::from_static(PEER_ID_HEADER_STR);

#[derive(Deserialize)]
struct PeerIdQuery {
    #[allow(dead_code)]
    peer_id: Option<String>,
}

pub struct PeerIdRejection {
    status: StatusCode,
    message: &'static str,
}

impl PeerIdRejection {
    fn missing_required() -> Self {
        warn!("`x-sf-peer-id` header or `peer_id` query parameter is required");
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "`x-sf-peer-id` header or `peer_id` query parameter is required",
        }
    }

    fn invalid_header_value() -> Self {
        warn!("Invalid UTF-8 in `x-sf-peer-id` header");
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "Invalid `x-sf-peer-id` header value",
        }
    }

    fn empty_header() -> Self {
        warn!("Received empty `x-sf-peer-id` header");
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "`x-sf-peer-id` cannot be empty",
        }
    }

    fn empty_query() -> Self {
        warn!("Received empty `peer_id` query parameter");
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "`peer_id` query parameter cannot be empty",
        }
    }

    fn bad_query_parse() -> Self {
        warn!("Failed to parse query parameters while looking for `peer_id`");
        Self {
            status: StatusCode::BAD_REQUEST,
            message: "Failed to parse query parameters",
        }
    }
}

impl IntoResponse for PeerIdRejection {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

impl<S> FromRequestParts<S> for ExtractPeerID
where
    S: Send + Sync,
{
    type Rejection = PeerIdRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        debug!("Extracting peer ID from request parts {}", parts.uri);
        if let Some(peer_id_header) = parts.headers.get(PEER_ID_HEADER) {
            return Self::extract_from_header(peer_id_header);
        }

        Self::extract_from_query(parts, state).await
    }
}

impl ExtractPeerID {
    fn extract_from_header(header_value: &HeaderValue) -> Result<Self, PeerIdRejection> {
        let peer_id_str = header_value
            .to_str()
            .map_err(|_| PeerIdRejection::invalid_header_value())?;

        if peer_id_str.is_empty() {
            return Err(PeerIdRejection::empty_header());
        }

        Ok(ExtractPeerID(peer_id_str.to_owned()))
    }

    async fn extract_from_query<S>(parts: &mut Parts, _state: &S) -> Result<Self, PeerIdRejection>
    where
        S: Send + Sync,
    {
        match parts.uri.query() {
            Some(query) => {
                let param = query.split('&').find_map(|pair| {
                    let (key, val) = pair.split_once('=')?;
                    (key == "peer_id").then_some(val)
                });

                match param {
                    Some(raw_value) => {
                        let decoded = percent_decode(raw_value.as_bytes())
                            .map_err(|_| PeerIdRejection::bad_query_parse())?;

                        if decoded.is_empty() {
                            return Err(PeerIdRejection::empty_query());
                        }
                        Ok(ExtractPeerID(decoded))
                    }
                    None => Err(PeerIdRejection::missing_required()),
                }
            }
            None => Err(PeerIdRejection::missing_required()),
        }
    }
}

fn percent_decode(input: &[u8]) -> Result<String, ()> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'%' => {
                if i + 2 >= input.len() {
                    return Err(());
                }
                let hi = from_hex(input[i + 1])?;
                let lo = from_hex(input[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn from_hex(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{HeaderMap, HeaderValue, Request, StatusCode},
        routing::get,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_handler(peer_id: ExtractPeerID) -> String {
        peer_id.0
    }

    fn setup() -> Router {
        Router::new().route("/test", get(test_handler))
    }

    #[test]
    fn percent_decode_invalid_utf8_is_err() {
        assert_eq!(percent_decode(b"%FF"), Err(()));
    }

    #[tokio::test]
    async fn test_extract_from_header_ok() {
        let app = setup();

        let request = Request::builder()
            .uri("/test")
            .header(PEER_ID_HEADER, "test-peer-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"test-peer-id");

        // assert!(logs_contain("Extracting peer ID from"))
    }

    #[tokio::test]
    async fn test_extract_from_header_err() {
        let app = setup();

        let request = Request::builder()
            .uri("/test")
            .header(PEER_ID_HEADER, "")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"`x-sf-peer-id` cannot be empty");
    }

    #[tokio::test]
    async fn test_extract_query_ok() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=test-peer-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"test-peer-id");
    }

    #[tokio::test]
    async fn test_extract_query_bad_query() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_iddeadbeef&foo=ba")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_peer_id_missing() {
        let app = setup();

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(
            &bytes[..],
            b"`x-sf-peer-id` header or `peer_id` query parameter is required"
        );
    }

    #[tokio::test]
    async fn test_extract_query_empty() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"`peer_id` query parameter cannot be empty");
    }

    #[tokio::test]
    async fn test_extract_query_other_param() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id_123=")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(
            &bytes[..],
            b"`x-sf-peer-id` header or `peer_id` query parameter is required"
        );
    }

    #[tokio::test]
    async fn test_invalid_utf8_header() {
        let app = setup();

        let mut headers = HeaderMap::new();
        let invalid_utf8: &[u8] = &[0xFF, 0xFE, 0xFD];
        headers.insert(
            PEER_ID_HEADER,
            HeaderValue::from_bytes(invalid_utf8).unwrap(),
        );

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let (mut parts, body) = request.into_parts();
        parts.headers = headers;
        let request = Request::from_parts(parts, body);

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"Invalid `x-sf-peer-id` header value");
    }

    #[tokio::test]
    async fn test_invalid_utf8_query() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=%F0%Fe%F")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"Failed to parse query parameters");
    }

    #[tokio::test]
    async fn test_invalid_utf8_query_invalid_hex() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=%FF%fx%F")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"Failed to parse query parameters");
    }

    #[tokio::test]
    async fn test_invalid_utf8_query_invalid_hex_test_1st_byte() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=%xF")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"Failed to parse query parameters");
    }

    #[tokio::test]
    async fn test_invalid_utf8_query_invalid_hex_test_2nd_byte() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=%")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"Failed to parse query parameters");
    }
}
