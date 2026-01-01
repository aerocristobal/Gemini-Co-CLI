use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiRequest {
    pub contents: Vec<Content>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub content: Content,
}

pub struct GeminiClient {
    api_key: String,
    client: reqwest::Client,
    model: String,
}

impl GeminiClient {
    pub fn new() -> Result<Self> {
        let api_key = env::var("GEMINI_API_KEY")
            .context("GEMINI_API_KEY environment variable not set")?;

        Ok(Self {
            api_key,
            client: reqwest::Client::new(),
            model: "gemini-1.5-pro-latest".to_string(),
        })
    }

    pub async fn send_message(
        &self,
        user_message: String,
        context: Vec<String>,
    ) -> Result<String> {
        // Build the prompt with context
        let mut full_prompt = String::new();

        if !context.is_empty() {
            full_prompt.push_str("Terminal Context:\n");
            for ctx in context.iter().rev().take(20).rev() {
                full_prompt.push_str(&format!("{}\n", ctx));
            }
            full_prompt.push_str("\n");
        }

        full_prompt.push_str(&format!("User: {}\n\nPlease provide a helpful response. If you want to execute a command in the terminal, format it as: EXECUTE: <command>", user_message));

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: full_prompt,
                }],
            }],
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Gemini API error: {}", error_text));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        if let Some(candidate) = gemini_response.candidates.first() {
            if let Some(part) = candidate.content.parts.first() {
                Ok(part.text.clone())
            } else {
                Err(anyhow::anyhow!("No text in Gemini response"))
            }
        } else {
            Err(anyhow::anyhow!("No candidates in Gemini response"))
        }
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
