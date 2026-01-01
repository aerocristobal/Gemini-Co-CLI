use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages an interactive Gemini CLI terminal session using PTY
pub struct GeminiTerminal {
    pty_pair: Arc<Mutex<portable_pty::PtyPair>>,
    _child: Box<dyn portable_pty::Child + Send>,
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
        let cmd = CommandBuilder::new("gemini");

        // Spawn the process in the PTY
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn gemini CLI. Is it installed?")?;

        Ok(Self {
            pty_pair: Arc::new(Mutex::new(pty_pair)),
            _child: child,
        })
    }

    /// Get reader for PTY output
    pub fn get_reader(&self) -> Box<dyn Read + Send> {
        let pty = self.pty_pair.blocking_lock();
        pty.master.try_clone_reader().expect("Failed to clone reader")
    }

    /// Take writer for PTY input (can only be called once)
    pub fn take_writer(&self) -> Box<dyn Write + Send> {
        let mut pty = self.pty_pair.blocking_lock();
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

/// Parse Gemini output for EXECUTE commands
pub fn extract_command(text: &str) -> Option<String> {
    // Look for "EXECUTE: <command>" pattern
    if let Some(pos) = text.find("EXECUTE:") {
        let command_part = &text[pos + 8..];
        let command = command_part.lines().next().unwrap_or("").trim().to_string();
        if !command.is_empty() {
            return Some(command);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command() {
        let text = "Sure! EXECUTE: ls -la";
        assert_eq!(extract_command(text), Some("ls -la".to_string()));

        let text_no_cmd = "Here's some help text";
        assert_eq!(extract_command(text_no_cmd), None);

        let text_multiline = "I recommend:\nEXECUTE: pwd";
        assert_eq!(extract_command(text_multiline), Some("pwd".to_string()));
    }
}
