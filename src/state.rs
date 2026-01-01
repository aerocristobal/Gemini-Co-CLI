use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::ssh::SshSession;

/// Represents a user session with both Gemini and SSH terminals
#[derive(Clone)]
pub struct Session {
    pub id: Uuid,
    pub ssh_session: Option<Arc<Mutex<SshSession>>>,
    pub ssh_output_buffer: Arc<RwLock<Vec<String>>>,
    pub pending_commands: Arc<Mutex<Vec<PendingCommand>>>,
    /// Channel to send SSH output to Gemini terminal
    pub ssh_to_gemini_tx: Option<Arc<Mutex<mpsc::UnboundedSender<String>>>>,
}

#[derive(Clone, Debug)]
pub struct PendingCommand {
    pub command: String,
    pub approved: bool,
    pub id: Uuid,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            ssh_session: None,
            ssh_output_buffer: Arc::new(RwLock::new(Vec::new())),
            pending_commands: Arc::new(Mutex::new(Vec::new())),
            ssh_to_gemini_tx: None,
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
            let _ = tx.send(output);
        }
    }

    /// Get recent SSH output for Gemini context
    pub async fn get_ssh_context(&self) -> Vec<String> {
        self.ssh_output_buffer.read().await.clone()
    }

    pub async fn add_pending_command(&self, command: String) -> Uuid {
        let cmd_id = Uuid::new_v4();
        let mut pending = self.pending_commands.lock().await;
        pending.push(PendingCommand {
            command,
            approved: false,
            id: cmd_id,
        });
        cmd_id
    }

    pub async fn approve_command(&self, cmd_id: Uuid) -> Option<String> {
        let mut pending = self.pending_commands.lock().await;
        if let Some(cmd) = pending.iter_mut().find(|c| c.id == cmd_id) {
            cmd.approved = true;
            Some(cmd.command.clone())
        } else {
            None
        }
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
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_session(&self) -> Session {
        let session = Session::new();
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session.clone());
        session
    }

    pub async fn get_session(&self, id: Uuid) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(&id).cloned()
    }

    pub async fn remove_session(&self, id: Uuid) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&id);
    }
}
