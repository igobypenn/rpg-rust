//! LLM client abstraction for code generation.

mod client;
mod config;
mod openai;

pub use client::LlmClient;
pub use config::LlmConfig;
pub use openai::OpenAIClient;
