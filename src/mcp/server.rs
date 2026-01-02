//! MCP server implementation for SSH tools.
//!
//! Provides SSH tool implementations with manual JSON-RPC handling.
//! Uses rmcp 0.12.0 model types for MCP-compliant responses.

use crate::mcp::approval::ApprovalChannel;
use crate::ssh::{SshConfig, SshSession};
use rmcp::model::{CallToolResult, Content, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// Shared SSH session state for MCP tools.
pub struct SshState {
    pub session: Option<Arc<Mutex<SshSession>>>,
    output_buffer: Arc<RwLock<Vec<String>>>,
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
        // Keep buffer manageable
        if buffer.len() > 100 {
            let remove_count = buffer.len() - 100;
            buffer.drain(0..remove_count);
        }
    }

    /// Get recent output lines.
    pub async fn get_recent_output(&self, lines: usize) -> Vec<String> {
        let buffer = self.output_buffer.read().await;
        let start = buffer.len().saturating_sub(lines);
        buffer[start..].to_vec()
    }
}

// ============================================================================
// Tool Parameter Types (with JsonSchema for automatic schema generation)
// ============================================================================

/// Parameters for ssh_connect tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SshConnectParams {
    /// SSH server hostname or IP address.
    pub host: String,
    /// SSH server port (default: 22).
    #[serde(default = "default_port")]
    pub port: u16,
    /// Username for authentication.
    pub username: String,
    /// Password for authentication (optional if using key).
    #[serde(default)]
    pub password: Option<String>,
    /// Private key for authentication (optional if using password).
    #[serde(default)]
    pub private_key: Option<String>,
}

fn default_port() -> u16 {
    22
}

/// Parameters for ssh_execute tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SshExecuteParams {
    /// The command to execute on the remote server.
    pub command: String,
    /// Timeout in seconds for user approval (default: 30).
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Whether to wait for command output before returning (default: true).
    #[serde(default = "default_wait")]
    pub wait_for_output: bool,
}

fn default_timeout() -> u64 {
    30
}

fn default_wait() -> bool {
    true
}

/// Parameters for ssh_read_output tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SshReadOutputParams {
    /// Number of recent output lines to retrieve (default: 50).
    #[serde(default = "default_lines")]
    pub lines: usize,
}

fn default_lines() -> usize {
    50
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert a schemars schema to the Arc<Map<String, Value>> format expected by rmcp.
fn schema_to_arc_map<T: JsonSchema>() -> Arc<Map<String, Value>> {
    let schema = schemars::schema_for!(T);
    let value = serde_json::to_value(schema).unwrap_or_default();
    if let Value::Object(map) = value {
        Arc::new(map)
    } else {
        Arc::new(Map::new())
    }
}

// ============================================================================
// MCP SSH Service
// ============================================================================

/// MCP service providing SSH tools for Gemini CLI integration.
pub struct McpSshService {
    pub session_id: Uuid,
    ssh_state: Arc<RwLock<SshState>>,
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

    /// Get server info for MCP initialization.
    pub fn get_server_info(&self) -> Value {
        json!({
            "name": "gemini-co-cli-mcp",
            "version": env!("CARGO_PKG_VERSION")
        })
    }

    /// List available tools with their schemas.
    pub fn list_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "ssh_connect".into(),
                description: Some("Connect to a remote SSH server. Must be called before executing commands.".into()),
                input_schema: schema_to_arc_map::<SshConnectParams>(),
                annotations: None,
                output_schema: None,
                meta: None,
                icons: None,
                title: None,
            },
            Tool {
                name: "ssh_execute".into(),
                description: Some("Execute a command on the connected SSH server. Requires user approval.".into()),
                input_schema: schema_to_arc_map::<SshExecuteParams>(),
                annotations: None,
                output_schema: None,
                meta: None,
                icons: None,
                title: Some("SSH Execute (requires approval)".into()),
            },
            Tool {
                name: "ssh_read_output".into(),
                description: Some("Read recent output from the SSH terminal session.".into()),
                input_schema: schema_to_arc_map::<SshReadOutputParams>(),
                annotations: None,
                output_schema: None,
                meta: None,
                icons: None,
                title: None,
            },
        ]
    }

    /// Call a tool by name with the given arguments.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> CallToolResult {
        match name {
            "ssh_connect" => self.tool_ssh_connect(arguments).await,
            "ssh_execute" => self.tool_ssh_execute(arguments).await,
            "ssh_read_output" => self.tool_ssh_read_output(arguments).await,
            _ => CallToolResult::error(vec![Content::text(format!(
                "Unknown tool: {}",
                name
            ))]),
        }
    }

    /// Connect to a remote SSH server.
    async fn tool_ssh_connect(&self, arguments: Value) -> CallToolResult {
        let params: SshConnectParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Invalid parameters: {}",
                    e
                ))]);
            }
        };

        let config = SshConfig {
            host: params.host.clone(),
            port: params.port,
            username: params.username.clone(),
            password: params.password.clone(),
            private_key: params.private_key.clone(),
        };

        match SshSession::connect(config).await {
            Ok(session) => {
                let mut state = self.ssh_state.write().await;
                state.session = Some(Arc::new(Mutex::new(session)));

                CallToolResult::success(vec![Content::text(format!(
                    "Successfully connected to {}@{}:{}",
                    params.username, params.host, params.port
                ))])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!(
                "Failed to connect: {}",
                e
            ))]),
        }
    }

    /// Execute a command on the connected SSH server.
    async fn tool_ssh_execute(&self, arguments: Value) -> CallToolResult {
        let params: SshExecuteParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Invalid parameters: {}",
                    e
                ))]);
            }
        };

        // Check if SSH is connected
        {
            let state = self.ssh_state.read().await;
            if state.session.is_none() {
                return CallToolResult::error(vec![Content::text(
                    "SSH not connected. Call ssh_connect first.",
                )]);
            }
        }

        // Request user approval
        let timeout = Duration::from_secs(params.timeout_seconds);
        let (approval_id, response_rx) = self
            .approval_channel
            .request_approval(params.command.clone())
            .await;

        // Wait for approval with timeout
        let approved = match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Channel closed - treat as rejection
                return CallToolResult::error(vec![Content::text(
                    "Approval channel closed unexpectedly.",
                )]);
            }
            Err(_) => {
                // Timeout
                self.approval_channel
                    .broadcast_rejection(approval_id)
                    .await;
                return CallToolResult::error(vec![Content::text(
                    "Approval timeout - command not executed.",
                )]);
            }
        };

        if !approved {
            return CallToolResult::error(vec![Content::text("Command rejected by user.")]);
        }

        // Execute the approved command
        let state = self.ssh_state.read().await;
        if let Some(ssh) = &state.session {
            let mut ssh_guard = ssh.lock().await;

            if let Err(e) = ssh_guard.execute_command(params.command.clone()).await {
                return CallToolResult::error(vec![Content::text(format!(
                    "Command execution failed: {}",
                    e
                ))]);
            }

            // If wait_for_output, read some output
            if params.wait_for_output {
                // Give the command time to produce output
                tokio::time::sleep(Duration::from_millis(500)).await;

                match ssh_guard.read_output().await {
                    Ok(Some(output)) => {
                        // Store in buffer
                        drop(ssh_guard);
                        drop(state);
                        self.ssh_state.read().await.add_output(output.clone()).await;

                        CallToolResult::success(vec![Content::text(format!(
                            "Command executed successfully.\nOutput:\n{}",
                            output
                        ))])
                    }
                    Ok(None) => CallToolResult::success(vec![Content::text(
                        "Command executed successfully. No immediate output.",
                    )]),
                    Err(e) => CallToolResult::success(vec![Content::text(format!(
                        "Command executed but failed to read output: {}",
                        e
                    ))]),
                }
            } else {
                CallToolResult::success(vec![Content::text("Command sent successfully.")])
            }
        } else {
            CallToolResult::error(vec![Content::text("SSH session lost.")])
        }
    }

    /// Read recent output from the SSH terminal session.
    async fn tool_ssh_read_output(&self, arguments: Value) -> CallToolResult {
        let params: SshReadOutputParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult::error(vec![Content::text(format!(
                    "Invalid parameters: {}",
                    e
                ))]);
            }
        };

        let state = self.ssh_state.read().await;
        let output = state.get_recent_output(params.lines).await;

        if output.is_empty() {
            CallToolResult::success(vec![Content::text("No recent output available.")])
        } else {
            CallToolResult::success(vec![Content::text(output.join(""))])
        }
    }

    /// Get a reference to the SSH state.
    pub fn get_ssh_state(&self) -> Arc<RwLock<SshState>> {
        self.ssh_state.clone()
    }
}
