//! LLM client trait definition.

use async_trait::async_trait;
use crate::error::Result;

#[async_trait]
pub trait LlmClient: Send + Sync {
    fn model(&self) -> &str;
    
    async fn complete(&self, system: &str, user: &str) -> Result<String>;
    
    async fn complete_json_raw(&self, system: &str, user: &str) -> Result<serde_json::Value>;
    
    async fn complete_with_retry(
        &self,
        system: &str,
        user: &str,
        max_retries: usize,
    ) -> Result<String> {
        let mut last_error = None;
        
        for attempt in 0..=max_retries {
            match self.complete(system, user).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = std::time::Duration::from_millis(100 * 2u64.pow(attempt as u32));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| crate::GeneratorError::NotImplemented(
            "No error recorded".to_string()
        )))
    }
}

