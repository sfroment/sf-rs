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
