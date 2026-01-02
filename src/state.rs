use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::mcp::{ApprovalChannel, McpSshService};
use crate::ssh::SshSession;

/// Shared MCP services indexed by session ID.
pub type McpServices = Arc<RwLock<HashMap<Uuid, Arc<McpSshService>>>>;

/// Represents a user session with both Gemini and SSH terminals
#[derive(Clone)]
pub struct Session {
    pub id: Uuid,
    pub ssh_session: Option<Arc<Mutex<SshSession>>>,
    pub ssh_output_buffer: Arc<RwLock<Vec<String>>>,
    /// Event-driven approval channel (replaces polling-based pending_commands)
    pub approval_channel: Arc<ApprovalChannel>,
    /// MCP service for this session
    pub mcp_service: Arc<McpSshService>,
    /// Channel to send SSH output to Gemini terminal
    pub ssh_to_gemini_tx: Option<Arc<Mutex<mpsc::UnboundedSender<String>>>>,
    /// Optional per-session Gemini API key (for web-based authentication)
    pub gemini_api_key: Option<String>,
}

impl Session {
    pub fn new(gemini_api_key: Option<String>) -> Self {
        let id = Uuid::new_v4();
        let approval_channel = Arc::new(ApprovalChannel::new());
        let mcp_service = Arc::new(McpSshService::new(id, approval_channel.clone()));

        Self {
            id,
            ssh_session: None,
            ssh_output_buffer: Arc::new(RwLock::new(Vec::new())),
            approval_channel,
            mcp_service,
            ssh_to_gemini_tx: None,
            gemini_api_key,
        }
    }

    /// Add SSH terminal output to the buffer
    pub async fn add_ssh_output(&self, output: String) {
        let mut buffer = self.ssh_output_buffer.write().await;
        buffer.push(output.clone());

        // Keep buffer size manageable (last 100 entries)
        if buffer.len() > 100 {
            let remove_count = buffer.len() - 100;
            buffer.drain(0..remove_count);
        }

        // Also send to Gemini terminal if connected
        if let Some(tx) = &self.ssh_to_gemini_tx {
            let tx = tx.lock().await;
            let _ = tx.send(output.clone());
        }

        // Also add to MCP SSH state for tool access
        let ssh_state = self.mcp_service.get_ssh_state();
        ssh_state.read().await.add_output(output).await;
    }

    /// Get recent SSH output for Gemini context
    pub async fn get_ssh_context(&self) -> Vec<String> {
        self.ssh_output_buffer.read().await.clone()
    }

    /// Get the approval channel for this session.
    pub fn get_approval_channel(&self) -> Arc<ApprovalChannel> {
        self.approval_channel.clone()
    }

    /// Get the MCP service for this session.
    pub fn get_mcp_service(&self) -> Arc<McpSshService> {
        self.mcp_service.clone()
    }

    /// Set the channel to send SSH output to Gemini
    pub async fn set_ssh_to_gemini_channel(&mut self, tx: mpsc::UnboundedSender<String>) {
        self.ssh_to_gemini_tx = Some(Arc::new(Mutex::new(tx)));
    }
}

/// Global application state
#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
    /// MCP services registry for tool access
    pub mcp_services: McpServices,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            mcp_services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_session(&self, gemini_api_key: Option<String>) -> Session {
        let session = Session::new(gemini_api_key);

        // Register MCP service
        {
            let mut mcp_services = self.mcp_services.write().await;
            mcp_services.insert(session.id, session.mcp_service.clone());
        }

        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session.clone());
        session
    }

    pub async fn get_session(&self, id: Uuid) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(&id).cloned()
    }

    pub async fn remove_session(&self, id: Uuid) {
        // Unregister MCP service
        {
            let mut mcp_services = self.mcp_services.write().await;
            mcp_services.remove(&id);
        }

        let mut sessions = self.sessions.write().await;
        sessions.remove(&id);
    }

    /// Get the MCP services registry.
    pub fn get_mcp_services(&self) -> McpServices {
        self.mcp_services.clone()
    }
}
