use axum::{
    extract::{FromRequestParts, Query},
    http::{HeaderName, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::warn;

pub struct ExtractPeerID(pub String);

const PEER_ID_HEADER: HeaderName = HeaderName::from_static("x-sf-peer-id");

#[derive(Deserialize)]
struct PeerIdQuery {
    peer_id: Option<String>,
}

pub struct PeerIdRejection {
    status: StatusCode,
    message: &'static str,
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
        if let Some(peer_id_header) = parts.headers.get(PEER_ID_HEADER) {
            if let Ok(peer_id_str) = peer_id_header.to_str() {
                if peer_id_str.is_empty() {
                    warn!("Received empty `x-sf-peer-id` header");
                    Err(PeerIdRejection {
                        status: StatusCode::BAD_REQUEST,
                        message: "`x-sf-peer-id` cannot be empty",
                    })
                } else {
                    Ok(ExtractPeerID(peer_id_str.to_owned()))
                }
            } else {
                warn!("Invalid UTF-8 in `x-sf-peer-id` header");
                Err(PeerIdRejection {
                    status: StatusCode::BAD_REQUEST,
                    message: "Invalid `x-sf-peer-id` header value",
                })
            }
        } else {
            match Query::<PeerIdQuery>::from_request_parts(parts, state).await {
                Ok(Query(query)) => {
                    if let Some(peer_id_query) = query.peer_id {
                        if peer_id_query.is_empty() {
                            warn!("Received empty `peer_id` query parameter");
                            Err(PeerIdRejection {
                                status: StatusCode::BAD_REQUEST,
                                message: "`peer_id` query parameter cannot be empty",
                            })
                        } else {
                            Ok(ExtractPeerID(peer_id_query))
                        }
                    } else {
                        warn!("`x-sf-peer-id` header or `peer_id` query parameter is required");
                        Err(PeerIdRejection {
                            status: StatusCode::BAD_REQUEST,
                            message: "`x-sf-peer-id` header or `peer_id` query parameter is required",
                        })
                    }
                }
                Err(_) => {
                    warn!("Failed to parse query parameters while looking for `peer_id`");
                    Err(PeerIdRejection {
                        status: StatusCode::BAD_REQUEST,
                        message: "Failed to parse query parameters",
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{HeaderMap, HeaderValue, Request, StatusCode, Uri},
        routing::get,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    async fn test_handler(peer_id: ExtractPeerID) -> String {
        peer_id.0
    }

    fn setup() -> Router {
        Router::new().route("/test", get(test_handler))
    }

    #[tokio::test]
    async fn test_extract_from_header() {
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
    }

    #[tokio::test]
    async fn test_extract_from_query() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=query-peer-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"query-peer-id");
    }

    #[tokio::test]
    async fn test_header_preferred_over_query() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=query-peer-id")
            .header(PEER_ID_HEADER, "header-peer-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], b"header-peer-id");
    }

    #[tokio::test]
    async fn test_missing_peer_id() {
        let app = setup();

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_empty_header_peer_id() {
        let app = setup();

        let request = Request::builder()
            .uri("/test")
            .header(PEER_ID_HEADER, "")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_empty_query_peer_id() {
        let app = setup();

        let request = Request::builder()
            .uri("/test?peer_id=")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    #[traced_test]
    async fn test_invalid_utf8_header() {
        let app = setup();

        let mut headers = HeaderMap::new();
        let invalid_utf8: &[u8] = &[0xFF, 0xFE, 0xFD];
        headers.insert(
            PEER_ID_HEADER,
            HeaderValue::from_bytes(invalid_utf8).unwrap(),
        );

        let request = Request::builder()
            .uri("/test")
            .method("GET")
            .extension(axum::extract::OriginalUri(Uri::from_static("/test")))
            .body(Body::empty())
            .unwrap();

        let (mut parts, body) = request.into_parts();
        parts.headers = headers;
        let request = Request::from_parts(parts, body);

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Verify the warning was logged
        assert!(logs_contain("Invalid UTF-8 in `x-sf-peer-id` header"));
    }

    #[tokio::test]
    #[traced_test]
    async fn test_failed_query_param_parsing() {
        async fn failing_query_handler(result: Result<ExtractPeerID, PeerIdRejection>) -> Response {
            match result {
                Ok(peer_id) => (StatusCode::OK, peer_id.0).into_response(),
                Err(rejection) => rejection.into_response(),
            }
        }

        let query_test_app = Router::new().route("/test_query_failure", get(failing_query_handler));

        let request = Request::builder()
            .uri("/test_query_failure?peer_id=abc&peer_id=def")
            .body(Body::empty())
            .unwrap();

        let response = query_test_app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        assert!(logs_contain(
            "Failed to parse query parameters while looking for `peer_id`"
        ));
    }
}
