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
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::gemini::GeminiTerminal;
use crate::mcp::ApprovalEvent;
use crate::ssh::{SshConfig, SshSession};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRequest {
    pub session_id: Option<String>,
    /// Optional API key for per-session Gemini authentication
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub success: bool,
    pub mcp_url: String,
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
    /// A command is awaiting approval (sent to frontend).
    CommandRequested {
        approval_id: String,
        command: String,
    },
    /// A command was approved (sent to frontend).
    CommandApproved { approval_id: String },
    /// A command was rejected (sent to frontend).
    CommandRejected { approval_id: String },
    /// User decision on a command (received from frontend).
    CommandDecision {
        approval_id: String,
        approved: bool,
    },
}

/// Create a new session
pub async fn create_session_handler(
    State(state): State<AppState>,
    Json(request): Json<SessionRequest>,
) -> Json<SessionResponse> {
    // Filter out empty API keys
    let api_key = request.api_key.filter(|k| !k.is_empty());
    let session = state.create_session(api_key).await;
    Json(SessionResponse {
        session_id: session.id.to_string(),
        success: true,
        mcp_url: format!("http://localhost:3000/mcp/{}", session.id),
    })
}

/// Handle SSH connection request
pub async fn ssh_connect_handler(
    State(state): State<AppState>,
    Json(request): Json<SshConnectRequest>,
) -> Json<ConnectResponse> {
    // Create session (SSH-only, no Gemini API key needed)
    let session = state.create_session(None).await;

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

    // Get the per-session API key (may be None)
    let api_key = session.gemini_api_key.clone();

    // Spawn Gemini CLI process with PTY, passing the session's API key
    let gemini = match GeminiTerminal::spawn(api_key) {
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
        tracing::error!(
            "Gemini CLI process exited immediately after spawn - authentication required"
        );
        // Send error message to WebSocket
        let error_msg = TerminalMessage::Output {
            data: format!(
                "\x1b[31m✗ Gemini CLI authentication required\x1b[0m\r\n\r\n\
                Please set GEMINI_API_KEY environment variable:\r\n\
                1. Get an API key from: \x1b[36mhttps://aistudio.google.com/apikey\x1b[0m\r\n\
                2. Set the environment variable in docker-compose.yml:\r\n\
                   \x1b[33m- GEMINI_API_KEY=your_api_key_here\x1b[0m\r\n\r\n\
                Or authenticate with OAuth by running:\r\n\
                   \x1b[33mdocker-compose exec gemini-co-cli gemini\x1b[0m\r\n\r\n\
                MCP server available for Gemini CLI at:\r\n\
                   \x1b[33mhttp://localhost:3000/mcp/{}\x1b[0m\r\n\r\n",
                session_id
            ),
        };
        let _ = ws_sender
            .send(Message::Text(serde_json::to_string(&error_msg).unwrap()))
            .await;
        return; // Exit early since process is not running
    }

    let ws_sender = Arc::new(Mutex::new(ws_sender));

    // Get PTY reader and writer
    let gemini_for_io = gemini_arc.lock().await;
    let mut reader = gemini_for_io.get_reader().await;
    let mut writer = gemini_for_io.take_writer().await;
    drop(gemini_for_io); // Release lock

    // Task to read from PTY and send to WebSocket
    // Note: Command detection is now handled via MCP tool calls, not text parsing
    let ws_sender_clone = ws_sender.clone();
    let mut output_task = tokio::task::spawn_blocking(move || {
        let mut buffer = vec![0u8; 4096];

        tracing::info!("Gemini PTY output task started");

        loop {
            match reader.read(&mut buffer) {
                Ok(n) if n > 0 => {
                    let output = String::from_utf8_lossy(&buffer[..n]).to_string();
                    tracing::debug!("Gemini PTY output: {} bytes", n);

                    // Send output to WebSocket
                    let msg = TerminalMessage::Output { data: output };
                    let json = serde_json::to_string(&msg).unwrap();

                    // Use blocking channel to send to async task
                    let rt = tokio::runtime::Handle::current();
                    let sender = ws_sender_clone.clone();
                    if rt
                        .block_on(async {
                            let mut s = sender.lock().await;
                            s.send(Message::Text(json)).await
                        })
                        .is_err()
                    {
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
            let msg_opt = rt.block_on(async { ws_receiver.next().await });

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
                                    gemini.resize(width as u16, height as u16).await
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

    // Wait for tasks to complete
    tokio::select! {
        _ = &mut output_task => {
            input_task.abort();
        }
        _ = &mut input_task => {
            output_task.abort();
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

/// Commands sent from WebSocket receiver to SSH handler
enum SshCommand {
    Input(String),
    Resize(u32, u32),
    Close,
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

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Create channel for sending commands to SSH handler
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<SshCommand>();

    // Clone the SSH session
    let ssh_session = session.ssh_session.clone();

    // Single task that owns the SSH session and handles both input and output
    let session_clone = session.clone();
    let mut ssh_task = tokio::spawn(async move {
        if let Some(ssh) = ssh_session {
            let mut ssh_guard = ssh.lock().await;

            loop {
                tokio::select! {
                    // Check for commands from WebSocket
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(SshCommand::Input(data)) => {
                                if let Err(e) = ssh_guard.send_input(data).await {
                                    tracing::error!("Error sending SSH input: {}", e);
                                    break;
                                }
                            }
                            Some(SshCommand::Resize(width, height)) => {
                                if let Err(e) = ssh_guard.resize(width, height).await {
                                    tracing::error!("Error resizing SSH terminal: {}", e);
                                }
                            }
                            Some(SshCommand::Close) | None => {
                                tracing::info!("SSH command channel closed");
                                break;
                            }
                        }
                    }

                    // Check for output from SSH (with timeout to allow command processing)
                    result = tokio::time::timeout(
                        tokio::time::Duration::from_millis(50),
                        ssh_guard.read_output()
                    ) => {
                        match result {
                            Ok(Ok(Some(output))) => {
                                // Add to session's SSH output buffer
                                session_clone.add_ssh_output(output.clone()).await;

                                // Send to WebSocket
                                let msg = TerminalMessage::Output { data: output };
                                let json = serde_json::to_string(&msg).unwrap();
                                if ws_sender.send(Message::Text(json)).await.is_err() {
                                    tracing::warn!("WebSocket closed, stopping SSH handler");
                                    break;
                                }
                            }
                            Ok(Ok(None)) => {
                                // No output available, continue
                            }
                            Ok(Err(e)) => {
                                tracing::error!("Error reading SSH output: {}", e);
                                let msg = TerminalMessage::Error {
                                    message: e.to_string(),
                                };
                                let json = serde_json::to_string(&msg).unwrap();
                                let _ = ws_sender.send(Message::Text(json)).await;
                                break;
                            }
                            Err(_) => {
                                // Timeout - no output, continue to process commands
                            }
                        }
                    }
                }
            }
        }
    });

    // Task to receive WebSocket messages and forward to SSH handler
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(terminal_msg) = serde_json::from_str::<TerminalMessage>(&text) {
                    match terminal_msg {
                        TerminalMessage::Input { data } => {
                            if cmd_tx.send(SshCommand::Input(data)).is_err() {
                                break;
                            }
                        }
                        TerminalMessage::Resize { width, height } => {
                            if cmd_tx.send(SshCommand::Resize(width, height)).is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Message::Close(_) = msg {
                let _ = cmd_tx.send(SshCommand::Close);
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut ssh_task => recv_task.abort(),
        _ = &mut recv_task => ssh_task.abort(),
    };
}

/// Handle command approval WebSocket
///
/// This WebSocket receives approval events via broadcast and forwards them
/// to the frontend. The frontend sends back decisions which are submitted
/// to the ApprovalChannel.
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

    // Subscribe to approval events from the broadcast channel
    let approval_channel = session.get_approval_channel();
    let event_receiver = approval_channel.subscribe();
    let event_stream = BroadcastStream::new(event_receiver);

    // Task to forward approval events to WebSocket
    let mut event_task = tokio::spawn(async move {
        tokio::pin!(event_stream);

        while let Some(result) = event_stream.next().await {
            if let Ok(event) = result {
                let msg = match event {
                    ApprovalEvent::CommandRequested {
                        approval_id,
                        command,
                    } => CommandMessage::CommandRequested {
                        approval_id,
                        command,
                    },
                    ApprovalEvent::CommandApproved { approval_id } => {
                        CommandMessage::CommandApproved { approval_id }
                    }
                    ApprovalEvent::CommandRejected { approval_id } => {
                        CommandMessage::CommandRejected { approval_id }
                    }
                };

                let json = serde_json::to_string(&msg).unwrap();
                if sender.send(Message::Text(json)).await.is_err() {
                    tracing::warn!("WebSocket closed, stopping approval event forwarding");
                    break;
                }
            }
        }
    });

    // Task to handle decisions from frontend
    let approval_channel = session.get_approval_channel();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(cmd_msg) = serde_json::from_str::<CommandMessage>(&text) {
                    if let CommandMessage::CommandDecision {
                        approval_id,
                        approved,
                    } = cmd_msg
                    {
                        if let Ok(id) = Uuid::parse_str(&approval_id) {
                            let delivered = approval_channel.submit_decision(id, approved).await;
                            if delivered {
                                tracing::info!(
                                    "Approval decision delivered: {} = {}",
                                    approval_id,
                                    approved
                                );
                            } else {
                                tracing::warn!(
                                    "Approval decision not found (may have timed out): {}",
                                    approval_id
                                );
                            }
                        }
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = &mut event_task => recv_task.abort(),
        _ = &mut recv_task => event_task.abort(),
    };
}
