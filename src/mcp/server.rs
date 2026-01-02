//! MCP server implementation for SSH tools.

use crate::mcp::approval::ApprovalChannel;
use crate::mcp::tools::{
    get_tool_definitions, SshConnectParams, SshExecuteParams, SshReadOutputParams,
};
use crate::ssh::{SshConfig, SshSession};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// Shared SSH session state for MCP tools.
pub struct SshState {
    pub session: Option<Arc<Mutex<SshSession>>>,
    pub output_buffer: Arc<RwLock<Vec<String>>>,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            session: None,
            output_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add output to the buffer.
    pub async fn add_output(&self, output: String) {
        let mut buffer = self.output_buffer.write().await;
        buffer.push(output);

        // Keep buffer size manageable
        if buffer.len() > 100 {
            let remove_count = buffer.len() - 100;
            buffer.drain(0..remove_count);
        }
    }

    /// Get recent output.
    pub async fn get_recent_output(&self, lines: usize) -> Vec<String> {
        let buffer = self.output_buffer.read().await;
        buffer.iter().rev().take(lines).rev().cloned().collect()
    }
}

/// MCP SSH service that handles tool calls.
pub struct McpSshService {
    pub session_id: Uuid,
    pub ssh_state: Arc<RwLock<SshState>>,
    pub approval_channel: Arc<ApprovalChannel>,
}

impl McpSshService {
    /// Create a new MCP SSH service.
    pub fn new(session_id: Uuid, approval_channel: Arc<ApprovalChannel>) -> Self {
        Self {
            session_id,
            ssh_state: Arc::new(RwLock::new(SshState::new())),
            approval_channel,
        }
    }

    /// Handle an MCP JSON-RPC request.
    pub async fn handle_request(&self, request: Value) -> Value {
        let id = request.get("id").cloned();
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");

        let result = match method {
            "initialize" => self.handle_initialize(&request).await,
            "tools/list" => self.handle_list_tools().await,
            "tools/call" => self.handle_call_tool(&request).await,
            _ => Err(McpError::MethodNotFound(method.to_string())),
        };

        match result {
            Ok(result) => json!({
                "jsonrpc": "2.0",
                "result": result,
                "id": id
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": e.code(),
                    "message": e.message()
                },
                "id": id
            }),
        }
    }

    /// Handle initialize request.
    async fn handle_initialize(&self, _request: &Value) -> Result<Value, McpError> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "gemini-co-cli-ssh",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    /// Handle tools/list request.
    async fn handle_list_tools(&self) -> Result<Value, McpError> {
        let tools = get_tool_definitions();
        Ok(json!({ "tools": tools }))
    }

    /// Handle tools/call request.
    async fn handle_call_tool(&self, request: &Value) -> Result<Value, McpError> {
        let params = request
            .get("params")
            .ok_or(McpError::InvalidParams("Missing params".to_string()))?;

        let tool_name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or(McpError::InvalidParams("Missing tool name".to_string()))?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        match tool_name {
            "ssh_connect" => self.tool_ssh_connect(arguments).await,
            "ssh_execute" => self.tool_ssh_execute(arguments).await,
            "ssh_read_output" => self.tool_ssh_read_output(arguments).await,
            _ => Err(McpError::ToolNotFound(tool_name.to_string())),
        }
    }

    /// Connect to an SSH server.
    async fn tool_ssh_connect(&self, arguments: Value) -> Result<Value, McpError> {
        let params: SshConnectParams = serde_json::from_value(arguments)
            .map_err(|e| McpError::InvalidParams(e.to_string()))?;

        let config = SshConfig {
            host: params.host.clone(),
            port: params.port,
            username: params.username.clone(),
            password: params.password,
            private_key: params.private_key,
        };

        match SshSession::connect(config).await {
            Ok(ssh_session) => {
                let mut state = self.ssh_state.write().await;
                state.session = Some(Arc::new(Mutex::new(ssh_session)));

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Successfully connected to {}:{} as {}",
                            params.host, params.port, params.username)
                    }]
                }))
            }
            Err(e) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Failed to connect: {}", e)
                }],
                "isError": true
            })),
        }
    }

    /// Execute a command on the SSH server (requires approval).
    async fn tool_ssh_execute(&self, arguments: Value) -> Result<Value, McpError> {
        let params: SshExecuteParams = serde_json::from_value(arguments)
            .map_err(|e| McpError::InvalidParams(e.to_string()))?;

        // Check if SSH is connected
        let state = self.ssh_state.read().await;
        if state.session.is_none() {
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "No SSH connection. Use ssh_connect first."
                }],
                "isError": true
            }));
        }
        drop(state);

        // Request approval from user
        tracing::info!(
            "Requesting approval for command: {}",
            params.command
        );

        let timeout = Duration::from_secs(params.timeout_seconds as u64);
        let approval_result = self
            .approval_channel
            .wait_for_approval(params.command.clone(), timeout)
            .await;

        match approval_result {
            Ok(true) => {
                // Approved - execute the command
                let state = self.ssh_state.read().await;
                if let Some(ssh) = &state.session {
                    let mut ssh_guard = ssh.lock().await;

                    match ssh_guard.execute_command(params.command.clone()).await {
                        Ok(_) => {
                            drop(ssh_guard);
                            drop(state);

                            // Wait briefly for output if requested
                            if params.wait_for_output {
                                tokio::time::sleep(Duration::from_millis(500)).await;

                                let state = self.ssh_state.read().await;
                                let output = state.get_recent_output(20).await;
                                let output_text = output.join("");

                                Ok(json!({
                                    "content": [{
                                        "type": "text",
                                        "text": format!("Command executed: {}\n\nOutput:\n{}",
                                            params.command, output_text)
                                    }]
                                }))
                            } else {
                                Ok(json!({
                                    "content": [{
                                        "type": "text",
                                        "text": format!("Command sent: {}", params.command)
                                    }]
                                }))
                            }
                        }
                        Err(e) => Ok(json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Command execution failed: {}", e)
                            }],
                            "isError": true
                        })),
                    }
                } else {
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": "SSH session disconnected"
                        }],
                        "isError": true
                    }))
                }
            }
            Ok(false) => {
                // Rejected by user
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Command rejected by user: {}", params.command)
                    }]
                }))
            }
            Err(e) => {
                // Timeout or error
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Approval failed: {}", e)
                    }],
                    "isError": true
                }))
            }
        }
    }

    /// Read recent output from the SSH terminal.
    async fn tool_ssh_read_output(&self, arguments: Value) -> Result<Value, McpError> {
        let params: SshReadOutputParams = serde_json::from_value(arguments)
            .map_err(|e| McpError::InvalidParams(e.to_string()))?;

        let state = self.ssh_state.read().await;
        let output = state.get_recent_output(params.lines).await;

        let output_text = if output.is_empty() {
            "No SSH output available".to_string()
        } else {
            output.join("")
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": output_text
            }]
        }))
    }

    /// Get the SSH state for external access (e.g., WebSocket handlers).
    pub fn get_ssh_state(&self) -> Arc<RwLock<SshState>> {
        self.ssh_state.clone()
    }
}

/// MCP error types.
#[derive(Debug)]
pub enum McpError {
    MethodNotFound(String),
    ToolNotFound(String),
    InvalidParams(String),
    InternalError(String),
}

impl McpError {
    fn code(&self) -> i32 {
        match self {
            McpError::MethodNotFound(_) => -32601,
            McpError::ToolNotFound(_) => -32602,
            McpError::InvalidParams(_) => -32602,
            McpError::InternalError(_) => -32603,
        }
    }

    fn message(&self) -> String {
        match self {
            McpError::MethodNotFound(m) => format!("Method not found: {}", m),
            McpError::ToolNotFound(t) => format!("Tool not found: {}", t),
            McpError::InvalidParams(p) => format!("Invalid params: {}", p),
            McpError::InternalError(e) => format!("Internal error: {}", e),
        }
    }
}
