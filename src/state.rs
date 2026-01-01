use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::ssh::SshSession;

/// Represents a user session with both terminal and Gemini connections
#[derive(Clone)]
pub struct Session {
    pub id: Uuid,
    pub ssh_session: Option<Arc<Mutex<SshSession>>>,
    pub terminal_output_history: Arc<RwLock<Vec<String>>>,
    pub gemini_context: Arc<RwLock<Vec<String>>>,
    pub pending_commands: Arc<Mutex<Vec<PendingCommand>>>,
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
            terminal_output_history: Arc::new(RwLock::new(Vec::new())),
            gemini_context: Arc::new(RwLock::new(Vec::new())),
            pending_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_terminal_output(&self, output: String) {
        let mut history = self.terminal_output_history.write().await;
        history.push(output.clone());

        // Also add to Gemini context for AI awareness
        let mut context = self.gemini_context.write().await;
        context.push(format!("Terminal output: {}", output));

        // Keep context size manageable (last 100 entries)
        if context.len() > 100 {
            let remove_count = context.len() - 100;
            context.drain(0..remove_count);
        }
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

    pub async fn get_context(&self) -> Vec<String> {
        self.gemini_context.read().await.clone()
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
