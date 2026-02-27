use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;

use super::LLMClient;
use crate::error::{AIError, AIResult};

/// OpenAI API client wrapper.
pub struct OpenAIClient {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
}

impl OpenAIClient {
    /// Create a new OpenAI client with optional API key override.
    pub fn new(api_key: Option<String>, model: String) -> Self {
        let client = if let Some(key) = api_key {
            let config = async_openai::config::OpenAIConfig::new().with_api_key(key);
            Client::with_config(config)
        } else {
            Client::new()
        };
        Self { client, model }
    }

    /// Create client with default model (gpt-4o).
    pub fn with_default_model() -> Self {
        Self::new(None, "gpt-4o".to_string())
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> AIResult<String> {
        let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

        if let Some(sys) = system {
            messages.push(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(sys)
                    .build()
                    .map_err(|e| AIError::Parsing(e.to_string()))?
                    .into(),
            );
        }

        messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()
                .map_err(|e| AIError::Parsing(e.to_string()))?
                .into(),
        );

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()
            .map_err(|e| AIError::Parsing(e.to_string()))?;

        let response = self.client.chat().create(request).await?;

        response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| AIError::Parsing("No response content".to_string()))
    }
}
