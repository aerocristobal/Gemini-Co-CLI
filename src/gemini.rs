use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// Manages an interactive Gemini CLI terminal session
pub struct GeminiTerminal {
    process: Child,
}

impl GeminiTerminal {
    /// Spawn a new interactive Gemini CLI process
    pub async fn spawn() -> Result<Self> {
        let process = Command::new("gemini")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn gemini CLI. Is it installed?")?;

        Ok(Self { process })
    }

    /// Get stdin handle for writing to Gemini
    pub fn stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.process.stdin.take()
    }

    /// Get stdout handle for reading from Gemini
    pub fn stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.process.stdout.take()
    }

    /// Get stderr handle for reading errors
    pub fn stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.process.stderr.take()
    }

    /// Kill the Gemini process
    pub async fn kill(mut self) -> Result<()> {
        self.process
            .kill()
            .await
            .context("Failed to kill gemini process")?;
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

/// Monitor Gemini output for commands and send them to a channel
pub async fn monitor_for_commands(
    mut stdout: tokio::process::ChildStdout,
    tx: mpsc::UnboundedSender<String>,
) {
    let reader = BufReader::new(&mut stdout);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        // Check each line for EXECUTE commands
        if let Some(command) = extract_command(&line) {
            let _ = tx.send(command);
        }
    }
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
