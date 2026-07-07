use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;

/// Parse a boolean env var, case-insensitive. Accepts "true", "1", "yes", "on".
fn parse_bool_env(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"),
        Err(_) => false,
    }
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("No API key configured")]
    NoApiKey,
    #[error("Empty response from LLM")]
    EmptyResponse,
    #[error("Concurrency limit exceeded")]
    ConcurrencyLimit,
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub max_concurrent: usize,
    pub reasoning: bool,
    pub debug_mode: bool,
    pub debug_file: Option<PathBuf>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            base_url: None,
            api_key: None,
            max_tokens: 4096,
            temperature: 0.7,
            max_concurrent: 3,
            reasoning: false,
            debug_mode: false,
            debug_file: None,
        }
    }
}

impl LlmConfig {
    pub fn from_env() -> std::result::Result<Self, LlmError> {
        let api_key = env::var("OPENAI_API_KEY").ok();
        let base_url = env::var("OPENAI_BASE_URL").ok();
        let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let max_concurrent = env::var("OPENAI_MAX_CONCURRENT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        let debug_mode = parse_bool_env("RPG_DEBUG");
        let debug_file = env::var("RPG_DEBUG_FILE").ok().map(PathBuf::from);
        let reasoning = parse_bool_env("OPENAI_REASONING");

        Ok(Self {
            model,
            base_url,
            api_key,
            max_tokens: 4096,
            temperature: 0.7,
            max_concurrent,
            reasoning,
            debug_mode,
            debug_file,
        })
    }

    pub fn openai_compatible(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: Some(base_url.into()),
            api_key: None,
            max_tokens: 4096,
            temperature: 0.7,
            max_concurrent: 3,
            reasoning: false,
            debug_mode: false,
            debug_file: None,
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    pub fn with_reasoning(mut self, enabled: bool) -> Self {
        self.reasoning = enabled;
        self
    }

    pub fn with_debug_mode(mut self, mode: bool) -> Self {
        self.debug_mode = mode;
        self
    }

    pub fn with_debug_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.debug_file = Some(path.into());
        self
    }

    fn base_url(&self) -> &str {
        self.base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }

    fn get_api_key(&self) -> std::result::Result<String, LlmError> {
        if let Some(ref key) = self.api_key {
            return Ok(key.clone());
        }
        env::var("OPENAI_API_KEY").map_err(|_| LlmError::NoApiKey)
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: usize,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: String,
}

pub struct OpenAIClient {
    client: Client,
    config: LlmConfig,
    semaphore: Arc<Semaphore>,
}

impl std::fmt::Debug for OpenAIClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIClient")
            .field("config", &self.config)
            .field("max_concurrent", &self.config.max_concurrent)
            .finish_non_exhaustive()
    }
}

impl OpenAIClient {
    pub fn new(config: LlmConfig) -> std::result::Result<Self, LlmError> {
        let client = Client::new();
        // max_concurrent=0 would create a zero-permit semaphore that blocks
        // forever — clamp to 1.
        let max_conc = config.max_concurrent.max(1);
        let semaphore = Arc::new(Semaphore::new(max_conc));
        Ok(Self {
            client,
            config,
            semaphore,
        })
    }

    fn build_request(&self, system: &str, user: &str) -> ChatRequest {
        ChatRequest {
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
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            reasoning_effort: if self.config.reasoning {
                Some("high".to_string())
            } else {
                None
            },
        }
    }

    async fn send_request(&self, request: ChatRequest) -> std::result::Result<String, LlmError> {
        let start = std::time::Instant::now();

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| LlmError::ConcurrencyLimit)?;

        let api_key = self.config.get_api_key()?;
        let url = format!("{}/chat/completions", self.config.base_url());

        // Build debug output
        let request_json = serde_json::to_string_pretty(&request).unwrap_or_default();
        let mut debug_output = String::new();
        debug_output.push_str(&format!("\n{}\n", "=".repeat(60)));
        debug_output.push_str("=== LLM REQUEST ===\n");
        debug_output.push_str(&format!(
            "Timestamp: {}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        ));
        debug_output.push_str(&format!("URL: {}\n", url));
        debug_output.push_str(&format!("Model: {}\n", request.model));
        debug_output.push_str(&format!("Max Tokens: {}\n", request.max_tokens));
        debug_output.push_str(&format!("Temperature: {}\n", request.temperature));
        if let Some(ref effort) = request.reasoning_effort {
            debug_output.push_str(&format!("Reasoning Effort: {}\n", effort));
        }
        debug_output.push_str("\nMessages:\n");
        for msg in &request.messages {
            let content_preview = if msg.content.len() > 200 {
                // Walk back to char boundary to avoid panicking on multi-byte UTF-8.
                let mut cut = 200;
                while cut > 0 && !msg.content.is_char_boundary(cut) {
                    cut -= 1;
                }
                format!("{}...", &msg.content[..cut])
            } else {
                msg.content.clone()
            };
            debug_output.push_str(&format!("  [{}] {}\n", msg.role, content_preview));
        }
        debug_output.push_str(&format!("\nFull Request JSON:\n{}\n", request_json));

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        // Build response debug output
        debug_output.push_str("\n=== LLM RESPONSE ===\n");
        debug_output.push_str(&format!("Status: {}\n", status));
        debug_output.push_str(&format!("Duration: {}ms\n", start.elapsed().as_millis()));
        debug_output.push_str(&format!("Length: {} chars\n", response_text.len()));

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            debug_output.push_str(&format!(
                "\nResponse JSON:\n{}\n",
                serde_json::to_string_pretty(&json).unwrap_or(response_text.clone())
            ));
        } else {
            debug_output.push_str(&format!("\nResponse Text:\n{}\n", response_text));
        }

        if self.config.debug_mode {
            tracing::debug!("{}", debug_output);
        }

        if let Some(ref debug_file) = self.config.debug_file {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(debug_file)
            {
                if let Err(e) = file.write_all(debug_output.as_bytes()) {
                    tracing::warn!("failed to write debug output: {}", e);
                }
            }
        }

        if !status.is_success() {
            return Err(LlmError::Api(format!("{}: {}", status, response_text)));
        }

        let chat_response: ChatResponse = serde_json::from_str(&response_text)?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or(LlmError::EmptyResponse)
    }

    pub async fn complete(
        &self,
        system: &str,
        user: &str,
    ) -> std::result::Result<String, LlmError> {
        let request = self.build_request(system, user);
        self.send_request(request).await
    }

    pub async fn complete_json<T: DeserializeOwned>(
        &self,
        system: &str,
        user: &str,
    ) -> std::result::Result<T, LlmError> {
        let content = self.complete(system, user).await?;
        let json_str = Self::extract_json(&content);
        if json_str.is_empty() {
            tracing::warn!("extract_json returned empty string");
            tracing::debug!(
                "Content ends with: {:?}",
                &content[content.len().saturating_sub(100)..]
            );
        }
        serde_json::from_str(json_str).map_err(|e| {
            tracing::error!("JSON parse error: {:?}", e);
            tracing::debug!("Input was: {:?}", json_str);
            LlmError::Json(e)
        })
    }

    fn extract_json(content: &str) -> &str {
        // The model emits thinking text followed by the JSON. Extract the last
        // valid JSON structure (object or array) from the content.

        // 1. A ```json fenced block, if present, holds the answer.
        if let Some(start) = content.find("```json\n") {
            let body = start + "```json\n".len();
            if let Some(end) = content[body..].find("\n```") {
                return content[body..body + end].trim();
            }
        }

        // 2. Otherwise scan candidate line-starts (back to front) for a `{` or
        //    `[`, and let serde validate + balance each candidate. serde's
        //    byte_offset() marks where the value ends.
        let mut offset = 0usize;
        let mut candidate_offsets: Vec<usize> = Vec::new();
        for line in content.lines() {
            if line.trim_start().starts_with(['{', '[']) {
                candidate_offsets.push(offset);
            }
            offset += line.len();
            if offset < content.len() {
                offset += 1; // newline
            }
        }

        for start in candidate_offsets.into_iter().rev() {
            let rest = content[start..].trim_start();
            let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<serde_json::Value>();
            if stream.next().is_some_and(|r| r.is_ok()) {
                return rest[..stream.byte_offset()].trim();
            }
        }

        content.trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_array_at_end() {
        let content = r#"1.  **Analyze the Repository:**
    *   **Name:** Database layer

["ConnectionConfiguration", "ConnectionEstablishment", "QueryExecution"]"#;

        let result = OpenAIClient::extract_json(content);
        assert_eq!(
            result,
            r#"["ConnectionConfiguration", "ConnectionEstablishment", "QueryExecution"]"#
        );
    }

    #[test]
    fn test_extract_json_object_at_end() {
        let content = r#"1.  **Analyze the Request:**
    *   **Role:** Senior Software Analyst.

{"entities": {"User": {"features": ["test"], "description": "test"}}}"#;

        let result = OpenAIClient::extract_json(content);
        assert_eq!(
            result,
            r#"{"entities": {"User": {"features": ["test"], "description": "test"}}}"#
        );
    }

    #[test]
    fn test_extract_json_in_code_block() {
        let content = r#"Some thinking text...

```json
{"key": "value"}
```

More text."#;

        let result = OpenAIClient::extract_json(content);
        assert_eq!(result, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_after_markdown_backticks() {
        let content = r#"8.  **Construct Output:**
    *   `["ConnectionManagement", "QueryExecution"]`

["ConnectionManagement", "QueryExecution"]"#;

        let result = OpenAIClient::extract_json(content);
        assert_eq!(result, r#"["ConnectionManagement", "QueryExecution"]"#);
    }

    #[test]
    fn test_extract_json_after_rust_code_block() {
        let content = r#"Usually, this file contains:
    ```rust
    pub struct Connection { ... }
    impl Connection {
        pub fn new() -> Self { ... }
        pub fn query(&self, sql: &str) -> ... { ... }
    }
    ```

    So the areas are:
    1.  `ConnectionManagement`
    2.  `QueryExecution`

    Let's go with:
    `["ConnectionManagement", "QueryExecution"]`

["ConnectionManagement", "QueryExecution"]"#;

        let result = OpenAIClient::extract_json(content);
        assert_eq!(result, r#"["ConnectionManagement", "QueryExecution"]"#);
    }
}
