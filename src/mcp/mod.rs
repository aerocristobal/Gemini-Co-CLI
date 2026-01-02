//! MCP (Model Context Protocol) server implementation for SSH tools.
//!
//! This module provides structured tool calls for SSH operations using
//! rmcp 0.12.0's declarative #[tool] macros. Replaces the fragile
//! "EXECUTE:" text pattern parsing with type-safe JSON-RPC.

mod approval;
pub mod http;
mod server;
mod tools;

pub use approval::{ApprovalChannel, ApprovalEvent};
pub use server::McpSshService;
