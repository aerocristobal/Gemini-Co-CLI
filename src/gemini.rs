use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages an interactive Gemini CLI terminal session using PTY
pub struct GeminiTerminal {
    pty_pair: Arc<Mutex<portable_pty::PtyPair>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
}

impl GeminiTerminal {
    /// Spawn a new interactive Gemini CLI process with PTY
    ///
    /// # Arguments
    /// * `session_api_key` - Optional per-session API key from web authentication
    pub fn spawn(session_api_key: Option<String>) -> Result<Self> {
        let pty_system = portable_pty::native_pty_system();

        // Create a PTY with initial size
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to create PTY")?;

        // Build command to run gemini CLI
        let mut cmd = CommandBuilder::new("gemini");

        // Set terminal type for proper PTY operation
        if let Ok(term) = std::env::var("TERM") {
            cmd.env("TERM", term);
        } else {
            cmd.env("TERM", "xterm-256color");
        }

        // Priority for API key: session key > environment variable
        // This allows per-session authentication from the web UI
        if let Some(ref key) = session_api_key {
            if !key.is_empty() {
                cmd.env("GEMINI_API_KEY", key);
                tracing::info!("Gemini CLI starting with per-session API key authentication");
            }
        } else if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            if !api_key.is_empty() {
                cmd.env("GEMINI_API_KEY", api_key);
                tracing::info!("Gemini CLI starting with environment API key authentication");
            } else {
                tracing::info!("No API key provided - Gemini CLI will show interactive authentication");
            }
        } else {
            tracing::info!("No API key provided - Gemini CLI will show interactive authentication");
        }

        // Pass through HOME for OAuth credential storage
        if let Ok(home) = std::env::var("HOME") {
            cmd.env("HOME", &home);
            tracing::debug!("HOME directory set to: {}", home);
        }

        // Pass XDG config directory for Gemini CLI credentials
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            cmd.env("XDG_CONFIG_HOME", xdg_config);
        }

        // Pass PATH to ensure gemini CLI can find node and other dependencies
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }

        // Spawn the process in the PTY
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn gemini CLI. Is it installed?")?;

        tracing::info!("Gemini CLI process spawned successfully");

        Ok(Self {
            pty_pair: Arc::new(Mutex::new(pty_pair)),
            child: Arc::new(Mutex::new(child)),
        })
    }

    /// Check if the child process is still running
    pub async fn is_running(&self) -> bool {
        let mut child = self.child.lock().await;
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    tracing::info!("Gemini CLI process exited successfully (exit code 0)");
                } else {
                    tracing::warn!(
                        "Gemini CLI process exited with non-zero status: {:?}",
                        status
                    );
                    tracing::warn!(
                        "This usually means Gemini CLI needs authentication. \
                        Set GEMINI_API_KEY or run 'docker-compose exec gemini-co-cli gemini' to authenticate via OAuth."
                    );
                }
                false
            }
            Ok(None) => true, // Still running
            Err(e) => {
                tracing::error!("Error checking Gemini CLI process status: {}", e);
                false
            }
        }
    }

    /// Get reader for PTY output
    pub async fn get_reader(&self) -> Box<dyn Read + Send> {
        let pty = self.pty_pair.lock().await;
        pty.master.try_clone_reader().expect("Failed to clone reader")
    }

    /// Take writer for PTY input (can only be called once)
    pub async fn take_writer(&self) -> Box<dyn Write + Send> {
        let pty = self.pty_pair.lock().await;
        pty.master.take_writer().expect("Failed to take writer")
    }

    /// Resize the PTY
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let pty = self.pty_pair.lock().await;
        pty.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to resize PTY")?;
        Ok(())
    }
}
