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
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::gemini::{extract_command, GeminiTerminal};
use crate::ssh::{SshConfig, SshSession};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRequest {
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub success: bool,
}

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
pub enum CommandMessage {
    CommandRequest { command: String, command_id: String },
    CommandApproval { command_id: String, approved: bool },
}

/// Create a new session
pub async fn create_session_handler(State(state): State<AppState>) -> Json<SessionResponse> {
    let session = state.create_session().await;
    Json(SessionResponse {
        session_id: session.id.to_string(),
        success: true,
    })
}

/// Handle SSH connection request
pub async fn ssh_connect_handler(
    State(state): State<AppState>,
    Json(request): Json<SshConnectRequest>,
) -> Json<ConnectResponse> {
    // Create session
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

/// Handle Gemini terminal WebSocket connection
pub async fn gemini_terminal_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| gemini_terminal_connection(socket, session_id, state))
}

async fn gemini_terminal_connection(socket: WebSocket, session_id: String, state: AppState) {
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

    // Spawn Gemini CLI process with PTY
    let gemini = match GeminiTerminal::spawn() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("Failed to spawn Gemini CLI: {}", e);
            return;
        }
    };

    // Keep gemini instance for resize operations and process monitoring
    let gemini_arc = Arc::new(Mutex::new(gemini));

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Check if process is still running after spawn
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    let gemini_check = gemini_arc.lock().await;
    let is_running = gemini_check.is_running().await;
    drop(gemini_check);

    if !is_running {
        tracing::error!("Gemini CLI process exited immediately after spawn - authentication required");
        // Send error message to WebSocket
        let error_msg = TerminalMessage::Output {
            data: format!(
                "\x1b[31m✗ Gemini CLI authentication required\x1b[0m\r\n\r\n\
                Please set GEMINI_API_KEY environment variable:\r\n\
                1. Get an API key from: \x1b[36mhttps://aistudio.google.com/apikey\x1b[0m\r\n\
                2. Set the environment variable in docker-compose.yml:\r\n\
                   \x1b[33m- GEMINI_API_KEY=your_api_key_here\x1b[0m\r\n\r\n\
                Or authenticate with OAuth by running:\r\n\
                   \x1b[33mdocker-compose exec gemini-co-cli gemini\x1b[0m\r\n\r\n"
            ),
        };
        let _ = ws_sender.send(Message::Text(serde_json::to_string(&error_msg).unwrap())).await;
        return; // Exit early since process is not running
    }

    let ws_sender = Arc::new(Mutex::new(ws_sender));

    // Get PTY reader and writer
    let gemini_for_io = gemini_arc.lock().await;
    let mut reader = gemini_for_io.get_reader();
    let mut writer = gemini_for_io.take_writer();
    drop(gemini_for_io); // Release lock

    // Channel for command requests from Gemini
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<String>();

    // Task to read from PTY and send to WebSocket
    let ws_sender_clone = ws_sender.clone();
    let mut output_task = tokio::task::spawn_blocking(move || {
        let mut buffer = vec![0u8; 4096];
        let mut line_buffer = String::new();

        tracing::info!("Gemini PTY output task started");

        loop {
            match reader.read(&mut buffer) {
                Ok(n) if n > 0 => {
                    let output = String::from_utf8_lossy(&buffer[..n]).to_string();
                    tracing::debug!("Gemini PTY output: {} bytes", n);

                    // Accumulate output for command detection
                    line_buffer.push_str(&output);

                    // Check for EXECUTE commands in accumulated buffer
                    if let Some(command) = extract_command(&line_buffer) {
                        tracing::info!("Gemini suggested command: {}", command);
                        let _ = cmd_tx.send(command);
                    }

                    // Keep only last 1000 chars to avoid unbounded growth
                    if line_buffer.len() > 1000 {
                        line_buffer = line_buffer.chars().skip(line_buffer.len() - 1000).collect();
                    }

                    // Send output to WebSocket
                    let msg = TerminalMessage::Output { data: output };
                    let json = serde_json::to_string(&msg).unwrap();

                    // Use blocking channel to send to async task
                    let rt = tokio::runtime::Handle::current();
                    let sender = ws_sender_clone.clone();
                    if rt.block_on(async {
                        let mut s = sender.lock().await;
                        s.send(Message::Text(json)).await
                    }).is_err() {
                        tracing::warn!("WebSocket closed, stopping Gemini PTY output");
                        break;
                    }
                }
                Ok(_) => {
                    tracing::warn!("Gemini PTY reached EOF - process may have exited");
                    break;
                }
                Err(e) => {
                    tracing::error!("Error reading from Gemini PTY: {}", e);
                    break;
                }
            }
        }
        tracing::info!("Gemini PTY output task ended");
    });

    // Task to handle WebSocket input and send to PTY
    let gemini_for_resize = gemini_arc.clone();
    let mut input_task = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();

        tracing::info!("Gemini PTY input task started");

        loop {
            // Receive from WebSocket in async context
            let msg_opt = rt.block_on(async {
                ws_receiver.next().await
            });

            match msg_opt {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(terminal_msg) = serde_json::from_str::<TerminalMessage>(&text) {
                        match terminal_msg {
                            TerminalMessage::Input { data } => {
                                tracing::debug!("Gemini PTY input: {} bytes", data.len());
                                if let Err(e) = writer.write_all(data.as_bytes()) {
                                    tracing::error!("Error writing to Gemini PTY: {}", e);
                                    break;
                                }
                                if let Err(e) = writer.flush() {
                                    tracing::error!("Error flushing Gemini PTY: {}", e);
                                    break;
                                }
                            }
                            TerminalMessage::Resize { width, height } => {
                                tracing::info!("Gemini PTY resize: {}x{}", width, height);
                                let gemini_resize = gemini_for_resize.clone();
                                let resize_result = rt.block_on(async {
                                    let gemini = gemini_resize.lock().await;
                                    gemini.resize(width as u16, height as u16)
                                });
                                if let Err(e) = resize_result {
                                    tracing::error!("Failed to resize Gemini PTY: {}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    tracing::info!("Gemini WebSocket closed");
                    break;
                }
                Some(Err(e)) => {
                    tracing::error!("Gemini WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        tracing::info!("Gemini PTY input task ended");
    });

    // Task to handle command approvals and execute on SSH terminal
    let session_clone = session.clone();
    let mut cmd_task = tokio::spawn(async move {
        while let Some(command) = cmd_rx.recv().await {
            // Add to pending commands
            let cmd_id = session_clone.add_pending_command(command.clone()).await;

            tracing::info!("Command pending approval: {} (ID: {})", command, cmd_id);
        }
    });

    // Wait for tasks to complete
    tokio::select! {
        _ = &mut output_task => {
            input_task.abort();
            cmd_task.abort();
        }
        _ = &mut input_task => {
            output_task.abort();
            cmd_task.abort();
        }
        _ = &mut cmd_task => {
            output_task.abort();
            input_task.abort();
        }
    };
}

/// Handle SSH terminal WebSocket connection
pub async fn ssh_terminal_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ssh_terminal_connection(socket, session_id, state))
}

async fn ssh_terminal_connection(socket: WebSocket, session_id: String, state: AppState) {
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
                        // Add to session's SSH output buffer
                        session_clone.add_ssh_output(output.clone()).await;

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
                                // Send raw input for user keystrokes
                                if let Err(e) = ssh_guard.send_input(data).await {
                                    tracing::error!("Error sending input: {}", e);
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

/// Handle command approval WebSocket
pub async fn command_approval_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| command_approval_connection(socket, session_id, state))
}

async fn command_approval_connection(socket: WebSocket, session_id: String, state: AppState) {
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

    // Monitor pending commands and send to WebSocket
    let session_clone = session.clone();
    let mut monitor_task = tokio::spawn(async move {
        loop {
            let pending = session_clone.pending_commands.lock().await;
            for cmd in pending.iter() {
                if !cmd.approved {
                    let msg = CommandMessage::CommandRequest {
                        command: cmd.command.clone(),
                        command_id: cmd.id.to_string(),
                    };
                    let json = serde_json::to_string(&msg).unwrap();
                    let _ = sender.send(Message::Text(json)).await;
                }
            }
            drop(pending);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });

    // Handle command approvals
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(cmd_msg) = serde_json::from_str::<CommandMessage>(&text) {
                    match cmd_msg {
                        CommandMessage::CommandApproval {
                            command_id,
                            approved,
                        } => {
                            if approved {
                                let cmd_uuid = Uuid::parse_str(&command_id).unwrap();
                                if let Some(command) = session.approve_command(cmd_uuid).await {
                                    // Execute on SSH terminal
                                    if let Some(ssh) = &session.ssh_session {
                                        let mut ssh_guard = ssh.lock().await;
                                        if let Err(e) = ssh_guard.execute_command(command).await {
                                            tracing::error!("Error executing approved command: {}", e);
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
    });

    tokio::select! {
        _ = &mut monitor_task => recv_task.abort(),
        _ = &mut recv_task => monitor_task.abort(),
    };
}
