mod openai;

use async_trait::async_trait;
pub use openai::OpenAIClient;

use crate::error::AIResult;

/// Trait for LLM clients.
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Complete a prompt with optional system message.
    async fn complete(&self, prompt: &str, system: Option<&str>) -> AIResult<String>;
}
