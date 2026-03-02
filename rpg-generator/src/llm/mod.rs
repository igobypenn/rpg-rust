//! LLM client abstraction for code generation.

mod client;
mod openai;
mod config;

pub use client::LlmClient;
pub use openai::OpenAIClient;
pub use config::LlmConfig;
