use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::gemini::GeminiClient;
use crate::ssh::{SshConfig, SshSession};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnectRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectResponse {
    pub session_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalMessage {
    Input { data: String },
    Resize { width: u32, height: u32 },
    Output { data: String },
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeminiWsMessage {
    UserMessage { content: String },
    GeminiResponse { content: String },
    CommandRequest { command: String, command_id: String },
    CommandApproval { command_id: String, approved: bool },
    CommandExecuted { command: String, output: String },
    Error { message: String },
}

/// Handle SSH connection request
pub async fn ssh_connect_handler(
    State(state): State<AppState>,
    Json(request): Json<SshConnectRequest>,
) -> Json<ConnectResponse> {
    // Create a new session
    let session = state.create_session().await;

    // Attempt to connect via SSH
    let ssh_config = SshConfig {
        host: request.host,
        port: request.port,
        username: request.username,
        password: request.password,
        private_key: request.private_key,
    };

    match SshSession::connect(ssh_config).await {
        Ok(ssh_session) => {
            // Store the SSH session
            if let Some(mut session_obj) = state.get_session(session.id).await {
                session_obj.ssh_session = Some(Arc::new(Mutex::new(ssh_session)));
                // Update the session in the state
                let mut sessions = state.sessions.write().await;
                sessions.insert(session.id, session_obj.clone());
            }

            Json(ConnectResponse {
                session_id: session.id.to_string(),
                success: true,
                error: None,
            })
        }
        Err(e) => {
            state.remove_session(session.id).await;
            Json(ConnectResponse {
                session_id: String::new(),
                success: false,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Handle terminal WebSocket connection
pub async fn terminal_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| terminal_ws_connection(socket, session_id, state))
}

async fn terminal_ws_connection(socket: WebSocket, session_id: String, state: AppState) {
    let session_uuid = match Uuid::parse_str(&session_id) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!("Invalid session ID: {}", session_id);
            return;
        }
    };

    let session = match state.get_session(session_uuid).await {
        Some(s) => s,
        None => {
            tracing::error!("Session not found: {}", session_id);
            return;
        }
    };

    let (mut sender, mut receiver) = socket.split();

    // Clone the SSH session for reading
    let ssh_session = session.ssh_session.clone();

    // Spawn a task to read from SSH and send to WebSocket
    let session_clone = session.clone();
    let mut send_task = tokio::spawn(async move {
        if let Some(ssh) = ssh_session {
            let mut ssh_guard = ssh.lock().await;
            loop {
                match ssh_guard.read_output().await {
                    Ok(Some(output)) => {
                        // Add to session history
                        session_clone.add_terminal_output(output.clone()).await;

                        // Send to WebSocket
                        let msg = TerminalMessage::Output { data: output };
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        tracing::error!("Error reading SSH output: {}", e);
                        let msg = TerminalMessage::Error {
                            message: e.to_string(),
                        };
                        let json = serde_json::to_string(&msg).unwrap();
                        let _ = sender.send(Message::Text(json)).await;
                        break;
                    }
                }
            }
        }
    });

    // Handle incoming messages from WebSocket
    let ssh_session = session.ssh_session.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(terminal_msg) = serde_json::from_str::<TerminalMessage>(&text) {
                    match terminal_msg {
                        TerminalMessage::Input { data } => {
                            if let Some(ssh) = &ssh_session {
                                let mut ssh_guard = ssh.lock().await;
                                if let Err(e) = ssh_guard.execute_command(data).await {
                                    tracing::error!("Error executing command: {}", e);
                                }
                            }
                        }
                        TerminalMessage::Resize { width, height } => {
                            if let Some(ssh) = &ssh_session {
                                let mut ssh_guard = ssh.lock().await;
                                if let Err(e) = ssh_guard.resize(width, height).await {
                                    tracing::error!("Error resizing terminal: {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };
}

/// Handle Gemini WebSocket connection
pub async fn gemini_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| gemini_ws_connection(socket, session_id, state))
}

async fn gemini_ws_connection(socket: WebSocket, session_id: String, state: AppState) {
    let session_uuid = match Uuid::parse_str(&session_id) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!("Invalid session ID: {}", session_id);
            return;
        }
    };

    let session = match state.get_session(session_uuid).await {
        Some(s) => s,
        None => {
            tracing::error!("Session not found: {}", session_id);
            return;
        }
    };

    let gemini_client = match GeminiClient::new() {
        Ok(client) => Arc::new(client),
        Err(e) => {
            tracing::error!("Failed to create Gemini client: {}", e);
            return;
        }
    };

    let (mut sender, mut receiver) = socket.split();

    // Handle incoming messages from WebSocket
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(ws_msg) = serde_json::from_str::<GeminiWsMessage>(&text) {
                match ws_msg {
                    GeminiWsMessage::UserMessage { content } => {
                        // Get terminal context
                        let context = session.get_context().await;

                        // Send to Gemini
                        match gemini_client.send_message(content, context).await {
                            Ok(response) => {
                                // Check if Gemini wants to execute a command
                                if let Some(command) = GeminiClient::extract_command(&response) {
                                    // Request user approval
                                    let cmd_id = session.add_pending_command(command.clone()).await;

                                    let msg = GeminiWsMessage::CommandRequest {
                                        command,
                                        command_id: cmd_id.to_string(),
                                    };
                                    let json = serde_json::to_string(&msg).unwrap();
                                    let _ = sender.send(Message::Text(json)).await;
                                } else {
                                    // Send response without command
                                    let msg = GeminiWsMessage::GeminiResponse {
                                        content: response,
                                    };
                                    let json = serde_json::to_string(&msg).unwrap();
                                    let _ = sender.send(Message::Text(json)).await;
                                }
                            }
                            Err(e) => {
                                let msg = GeminiWsMessage::Error {
                                    message: e.to_string(),
                                };
                                let json = serde_json::to_string(&msg).unwrap();
                                let _ = sender.send(Message::Text(json)).await;
                            }
                        }
                    }
                    GeminiWsMessage::CommandApproval {
                        command_id,
                        approved,
                    } => {
                        if approved {
                            let cmd_uuid = Uuid::parse_str(&command_id).unwrap();
                            if let Some(command) = session.approve_command(cmd_uuid).await {
                                // Execute the command via SSH
                                if let Some(ssh) = &session.ssh_session {
                                    let mut ssh_guard = ssh.lock().await;
                                    if let Err(e) = ssh_guard.execute_command(command.clone()).await
                                    {
                                        let msg = GeminiWsMessage::Error {
                                            message: format!("Failed to execute command: {}", e),
                                        };
                                        let json = serde_json::to_string(&msg).unwrap();
                                        let _ = sender.send(Message::Text(json)).await;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
