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
    pub fn spawn() -> Result<Self> {
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

        // Ensure environment variables are passed through
        // This is critical for GEMINI_API_KEY authentication
        if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            cmd.env("GEMINI_API_KEY", api_key);
            tracing::info!("Gemini CLI starting with API key authentication");
        } else {
            tracing::warn!("GEMINI_API_KEY not set - Gemini CLI may require authentication");
        }

        // Pass through HOME for OAuth credential storage
        if let Ok(home) = std::env::var("HOME") {
            cmd.env("HOME", home);
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
                tracing::warn!("Gemini CLI process exited with status: {:?}", status);
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
    pub fn get_reader(&self) -> Box<dyn Read + Send> {
        let pty = self.pty_pair.blocking_lock();
        pty.master.try_clone_reader().expect("Failed to clone reader")
    }

    /// Take writer for PTY input (can only be called once)
    pub fn take_writer(&self) -> Box<dyn Write + Send> {
        let pty = self.pty_pair.blocking_lock();
        pty.master.take_writer().expect("Failed to take writer")
    }

    /// Resize the PTY
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let pty = self.pty_pair.blocking_lock();
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
