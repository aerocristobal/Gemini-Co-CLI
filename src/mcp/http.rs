//! HTTP handlers for MCP server endpoints.
//!
//! Provides JSON-RPC and SSE endpoints for MCP tool communication.

use crate::mcp::approval::ApprovalEvent;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::stream::Stream;
use serde_json::Value;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

/// Handle MCP JSON-RPC requests.
///
/// POST /mcp/:session_id
pub async fn mcp_handler(
    Path(session_id): Path<String>,
    State(app_state): State<AppState>,
    Json(request): Json<Value>,
) -> Response {
    let session_uuid = match Uuid::parse_str(&session_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32600,
                        "message": "Invalid session ID"
                    },
                    "id": request.get("id").cloned()
                })),
            )
                .into_response();
        }
    };

    let services = app_state.mcp_services.read().await;
    let service = match services.get(&session_uuid) {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32001,
                        "message": "Session not found"
                    },
                    "id": request.get("id").cloned()
                })),
            )
                .into_response();
        }
    };
    drop(services);

    let response = service.handle_request(request).await;
    Json(response).into_response()
}

/// Handle MCP SSE event stream for approval events.
///
/// GET /mcp/:session_id/events
pub async fn mcp_sse_handler(
    Path(session_id): Path<String>,
    State(app_state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let session_uuid = Uuid::parse_str(&session_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let services = app_state.mcp_services.read().await;
    let service = services
        .get(&session_uuid)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    drop(services);

    // Subscribe to approval events
    let receiver = service.approval_channel.subscribe();
    let stream = BroadcastStream::new(receiver);

    let event_stream = stream.filter_map(|result| {
        match result {
            Ok(event) => {
                let event_json = serde_json::to_string(&event).ok()?;
                let event_type = match &event {
                    ApprovalEvent::CommandRequested { .. } => "command_requested",
                    ApprovalEvent::CommandApproved { .. } => "command_approved",
                    ApprovalEvent::CommandRejected { .. } => "command_rejected",
                };
                Some(Ok(Event::default().event(event_type).data(event_json)))
            }
            Err(_) => None, // Skip lagged messages
        }
    });

    Ok(Sse::new(event_stream))
}
