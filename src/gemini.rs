use anyhow::{Context, Result};
use tokio::process::Command;

pub struct GeminiClient {
    model: String,
}

impl GeminiClient {
    pub fn new() -> Result<Self> {
        // Uses official @google/gemini-cli
        // Authentication via GEMINI_API_KEY env var or interactive OAuth
        Ok(Self {
            model: "gemini-1.5-pro".to_string(),
        })
    }

    /// Check if Gemini CLI is available and authenticated
    /// Note: Official CLI doesn't have an explicit auth status command
    /// We check by attempting a simple query
    pub async fn check_auth() -> Result<bool> {
        let output = Command::new("gemini")
            .arg("-p")
            .arg("Hello")
            .arg("--output-format")
            .arg("json")
            .output()
            .await
            .context("Failed to check Gemini CLI. Is it installed?")?;

        Ok(output.status.success())
    }

    pub async fn send_message(
        &self,
        user_message: String,
        context: Vec<String>,
    ) -> Result<String> {
        // Build the prompt with context
        let mut full_prompt = String::new();

        if !context.is_empty() {
            full_prompt.push_str("Terminal Context (recent output):\n```\n");
            for ctx in context.iter().rev().take(20).rev() {
                full_prompt.push_str(&format!("{}\n", ctx));
            }
            full_prompt.push_str("```\n\n");
        }

        full_prompt.push_str(&format!(
            "{}\n\nIMPORTANT: If you want to execute a command in the terminal, format it as: EXECUTE: <command>",
            user_message
        ));

        // Execute gemini with non-interactive prompt
        // Official CLI syntax: gemini -p "prompt" -m model-name
        let output = Command::new("gemini")
            .arg("-p")
            .arg(&full_prompt)
            .arg("-m")
            .arg(&self.model)
            .output()
            .await
            .context("Failed to execute gemini CLI. Ensure it's installed and authenticated.")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Gemini CLI error: {}. Authentication required. Set GEMINI_API_KEY or run 'docker run -it <container> gemini' to login with Google.",
                error
            ));
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();

        // Clean up the response (remove any CLI formatting)
        let cleaned_response = response.trim().to_string();

        if cleaned_response.is_empty() {
            return Err(anyhow::anyhow!("Empty response from Gemini CLI"));
        }

        Ok(cleaned_response)
    }

    /// Extract command from Gemini response if present
    pub fn extract_command(response: &str) -> Option<String> {
        if let Some(pos) = response.find("EXECUTE:") {
            let command_part = &response[pos + 8..];
            let command = command_part
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !command.is_empty() {
                return Some(command);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command() {
        let response = "I'll help you with that. EXECUTE: ls -la";
        assert_eq!(
            GeminiClient::extract_command(response),
            Some("ls -la".to_string())
        );

        let response_no_cmd = "I'll help you with that.";
        assert_eq!(GeminiClient::extract_command(response_no_cmd), None);
    }
}
