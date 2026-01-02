//! HTTP handlers for MCP server endpoints.
//!
//! Provides JSON-RPC and SSE endpoints for MCP tool communication.
//! Uses rmcp 0.12.0 model types for MCP-compliant responses.

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
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

/// JSON-RPC request structure.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<Value>,
    id: Option<Value>,
}

/// JSON-RPC response structure.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

/// Handle MCP JSON-RPC requests.
///
/// POST /mcp/:session_id
pub async fn mcp_handler(
    Path(session_id): Path<String>,
    State(app_state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> Response {
    let session_uuid = match Uuid::parse_str(&session_id) {
        Ok(id) => id,
        Err(_) => {
            return Json(JsonRpcResponse::error(
                request.id,
                -32600,
                "Invalid session ID".to_string(),
            ))
            .into_response();
        }
    };

    let services = app_state.mcp_services.read().await;
    let service = match services.get(&session_uuid) {
        Some(s) => s.clone(),
        None => {
            return Json(JsonRpcResponse::error(
                request.id,
                -32001,
                "Session not found".to_string(),
            ))
            .into_response();
        }
    };
    drop(services);

    // Handle different MCP methods
    let response = match request.method.as_str() {
        "initialize" => {
            let info = service.get_server_info();
            JsonRpcResponse::success(
                request.id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": info,
                    "capabilities": {
                        "tools": {}
                    }
                }),
            )
        }

        "tools/list" => {
            let tools = service.list_tools();
            let tools_json: Vec<Value> = tools
                .into_iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema
                    })
                })
                .collect();

            JsonRpcResponse::success(request.id, json!({ "tools": tools_json }))
        }

        "tools/call" => {
            let params = request.params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = service.call_tool(tool_name, arguments).await;

            // Serialize the Content using serde (rmcp types are serializable)
            let content_json = serde_json::to_value(&result.content).unwrap_or_default();

            JsonRpcResponse::success(
                request.id,
                json!({
                    "content": content_json,
                    "isError": result.is_error.unwrap_or(false)
                }),
            )
        }

        "notifications/initialized" => {
            // Client notification that initialization is complete
            JsonRpcResponse::success(request.id, json!({}))
        }

        _ => JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    };

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

    let event_stream = stream.filter_map(|result| match result {
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
    });

    Ok(Sse::new(event_stream))
}
