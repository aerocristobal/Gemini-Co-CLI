//! MCP tool parameter schemas for SSH operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for the ssh_connect tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SshConnectParams {
    /// SSH server hostname or IP address.
    pub host: String,

    /// SSH server port (default: 22).
    #[serde(default = "default_port")]
    pub port: u16,

    /// Username for SSH authentication.
    pub username: String,

    /// Password for authentication (optional if using key).
    #[serde(default)]
    pub password: Option<String>,

    /// Private key content for key-based authentication.
    #[serde(default)]
    pub private_key: Option<String>,
}

fn default_port() -> u16 {
    22
}

/// Parameters for the ssh_execute tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SshExecuteParams {
    /// Command to execute on the remote SSH server.
    pub command: String,

    /// Timeout in seconds for command execution (default: 30).
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,

    /// Whether to wait for and return command output (default: true).
    #[serde(default = "default_wait")]
    pub wait_for_output: bool,
}

fn default_timeout() -> u32 {
    30
}

fn default_wait() -> bool {
    true
}

/// Parameters for the ssh_read_output tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SshReadOutputParams {
    /// Number of recent lines to retrieve (default: 50).
    #[serde(default = "default_lines")]
    pub lines: usize,
}

fn default_lines() -> usize {
    50
}

/// Response status for ssh_execute tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteStatus {
    /// Command is awaiting user approval.
    AwaitingApproval,
    /// Command was approved and executed.
    Executed,
    /// Command was rejected by user.
    Rejected,
    /// An error occurred.
    Error,
}

/// Response for ssh_execute tool.
#[derive(Debug, Clone, Serialize)]
pub struct ExecuteResponse {
    pub status: ExecuteStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Tool definition for MCP protocol.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Generate tool definitions for the MCP server.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "ssh_connect".to_string(),
            description: "Connect to a remote SSH server. Must be called before executing commands.".to_string(),
            input_schema: serde_json::to_value(schemars::schema_for!(SshConnectParams)).unwrap(),
        },
        ToolDefinition {
            name: "ssh_execute".to_string(),
            description: "Execute a command on the connected SSH server. Requires user approval before execution.".to_string(),
            input_schema: serde_json::to_value(schemars::schema_for!(SshExecuteParams)).unwrap(),
        },
        ToolDefinition {
            name: "ssh_read_output".to_string(),
            description: "Read recent output from the SSH terminal session.".to_string(),
            input_schema: serde_json::to_value(schemars::schema_for!(SshReadOutputParams)).unwrap(),
        },
    ]
}
