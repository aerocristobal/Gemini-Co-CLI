//! MCP (Model Context Protocol) server implementation for SSH tools.
//!
//! This module provides structured tool calls for SSH operations,
//! replacing the fragile "EXECUTE:" text pattern parsing.

mod approval;
pub mod http;
mod server;
mod tools;

pub use approval::{ApprovalChannel, ApprovalEvent};
pub use server::McpSshService;
