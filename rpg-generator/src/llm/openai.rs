//! OpenAI client implementation.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Semaphore;

use super::{LlmClient, LlmConfig};
use crate::error::{GeneratorError, Result};

pub struct OpenAIClient {
    client: Client,
    config: LlmConfig,
    semaphore: std::sync::Arc<Semaphore>,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}
#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

impl OpenAIClient {
    pub fn new(config: LlmConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| GeneratorError::Http(e.to_string()))?;

        let semaphore = std::sync::Arc::new(Semaphore::new(config.max_concurrent));

        Ok(Self {
            client,
            config,
            semaphore,
        })
    }

    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }

    fn extract_json(&self, content: &str) -> Result<serde_json::Value> {
        let content = content.trim();

        if content.starts_with('{') || content.starts_with('[') {
            return serde_json::from_str(content).map_err(|e| GeneratorError::LlmFailure {
                phase: crate::types::Phase::FeaturePlanning,
                reason: format!("JSON parse error: {}", e),
                raw_output: Some(content.to_string()),
            });
        }

        let start = content
            .find('{')
            .or_else(|| content.find('['))
            .ok_or_else(|| GeneratorError::LlmFailure {
                phase: crate::types::Phase::FeaturePlanning,
                reason: "No JSON found in response".to_string(),
                raw_output: Some(content.to_string()),
            })?;

        let end = content
            .rfind('}')
            .or_else(|| content.rfind(']'))
            .ok_or_else(|| GeneratorError::LlmFailure {
                phase: crate::types::Phase::FeaturePlanning,
                reason: "No closing brace found in response".to_string(),
                raw_output: Some(content.to_string()),
            })?;

        let json_str = &content[start..=end];
        serde_json::from_str(json_str).map_err(|e| GeneratorError::LlmFailure {
            phase: crate::types::Phase::FeaturePlanning,
            reason: format!("JSON parse error: {}", e),
            raw_output: Some(content.to_string()),
        })
    }

    /// Deserialize a JSON value into the target type.
    pub fn deserialize<T: serde::de::DeserializeOwned>(
        &self,
        value: serde_json::Value,
    ) -> Result<T> {
        serde_json::from_value(value).map_err(|e| GeneratorError::LlmFailure {
            phase: crate::types::Phase::FeaturePlanning,
            reason: format!("JSON parse error: {}", e),
            raw_output: None,
        })
    }
}

#[async_trait]
impl LlmClient for OpenAIClient {
    fn model(&self) -> &str {
        &self.config.model
    }

    async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let _permit =
            self.semaphore.acquire().await.map_err(|e| {
                GeneratorError::Infrastructure(std::io::Error::other(e.to_string()))
            })?;

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
            reasoning_effort: if self.config.reasoning {
                Some("high".to_string())
            } else {
                None
            },
        };

        let url = format!("{}/chat/completions", self.base_url());

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| GeneratorError::LlmFailure {
                phase: crate::types::Phase::FeaturePlanning,
                reason: format!("HTTP error: {}", e),
                raw_output: None,
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GeneratorError::LlmFailure {
                phase: crate::types::Phase::FeaturePlanning,
                reason: format!("API error {}: {}", status, body),
                raw_output: Some(body),
            });
        }

        let chat_response: ChatResponse =
            response
                .json()
                .await
                .map_err(|e| GeneratorError::LlmFailure {
                    phase: crate::types::Phase::FeaturePlanning,
                    reason: format!("Response parse error: {}", e),
                    raw_output: None,
                })?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| GeneratorError::LlmFailure {
                phase: crate::types::Phase::FeaturePlanning,
                reason: "No choices in response".to_string(),
                raw_output: None,
            })
    }

    async fn complete_json_raw(&self, system: &str, user: &str) -> Result<serde_json::Value> {
        let content = self.complete(system, user).await?;
        self.extract_json(&content)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json() {
        let config = LlmConfig::new("test-key");
        let client = OpenAIClient::new(config).unwrap();

        let json: serde_json::Value = client.extract_json(r#"{"key": "value"}"#).unwrap();
        assert_eq!(json["key"], "value");

        let json: serde_json::Value = client
            .extract_json(r#"Some text before {"key": "value"} and after"#)
            .unwrap();
        assert_eq!(json["key"], "value");
    }
}
