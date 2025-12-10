use async_trait::async_trait;
use base64::prelude::*;
use rig::{
    completion::Prompt,
    message::{ImageDetail, ImageMediaType, Message, UserContent},
    providers::{
        deepseek,
        gemini,
        openai,
    },
    OneOrMany,
};
use rig::prelude::*;
use std::sync::Arc;

use crate::error::MarkitdownError;

/// Trait for LLM clients that can describe images
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate a description for an image
    async fn describe_image(&self, image_data: &[u8], mime_type: &str) -> Result<String, MarkitdownError>;

    /// Generate a description for an image from base64
    async fn describe_image_base64(&self, base64_data: &str, mime_type: &str) -> Result<String, MarkitdownError>;

    /// Generate a completion for a text prompt
    async fn complete(&self, prompt: &str) -> Result<String, MarkitdownError>;
}

/// Supported LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    Gemini,
    OpenAI,
    DeepSeek,
}

impl std::str::FromStr for LlmProvider {
    type Err = MarkitdownError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gemini" | "google" => Ok(LlmProvider::Gemini),
            "openai" | "gpt" => Ok(LlmProvider::OpenAI),
            "deepseek" => Ok(LlmProvider::DeepSeek),
            _ => Err(MarkitdownError::LlmError(format!("Unknown LLM provider: {}", s))),
        }
    }
}

/// Configuration for LLM clients
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub temperature: f64,
    pub preamble: String,
}

impl LlmConfig {
    pub fn new(provider: LlmProvider, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
            temperature: 0.5,
            preamble: "You are an image describer. Provide detailed, accurate descriptions of images.".to_string(),
        }
    }

    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_preamble(mut self, preamble: impl Into<String>) -> Self {
        self.preamble = preamble.into();
        self
    }

    /// Create a Gemini configuration with default model
    pub fn gemini() -> Self {
        Self::new(LlmProvider::Gemini, "gemini-1.5-flash")
    }

    /// Create an OpenAI configuration with default model
    pub fn openai() -> Self {
        Self::new(LlmProvider::OpenAI, "gpt-4o-mini")
    }

    /// Create a DeepSeek configuration with default model
    pub fn deepseek() -> Self {
        Self::new(LlmProvider::DeepSeek, "deepseek-chat")
    }
}

/// Generic LLM client that can use different providers
pub struct GenericLlmClient {
    config: LlmConfig,
}

impl GenericLlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }

    pub fn from_env(provider: LlmProvider, model: impl Into<String>) -> Self {
        Self::new(LlmConfig::new(provider, model))
    }

    /// Create from provider string and model
    pub fn from_strings(provider: &str, model: &str) -> Result<Self, MarkitdownError> {
        let provider = provider.parse()?;
        Ok(Self::new(LlmConfig::new(provider, model)))
    }
}

fn parse_mime_to_image_type(mime_type: &str) -> ImageMediaType {
    match mime_type.to_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ImageMediaType::JPEG,
        "image/png" => ImageMediaType::PNG,
        "image/gif" => ImageMediaType::GIF,
        "image/webp" => ImageMediaType::WEBP,
        _ => ImageMediaType::JPEG, // Default fallback
    }
}

#[async_trait]
impl LlmClient for GenericLlmClient {
    async fn describe_image(&self, image_data: &[u8], mime_type: &str) -> Result<String, MarkitdownError> {
        let base64_data = BASE64_STANDARD.encode(image_data);
        self.describe_image_base64(&base64_data, mime_type).await
    }

    async fn describe_image_base64(&self, base64_data: &str, mime_type: &str) -> Result<String, MarkitdownError> {
        let image_type = parse_mime_to_image_type(mime_type);

        match self.config.provider {
            LlmProvider::Gemini => {
                let client = gemini::Client::from_env();
                let agent = client
                    .agent(&self.config.model)
                    .preamble(&self.config.preamble)
                    .temperature(self.config.temperature)
                    .build();

                let mut content_items = OneOrMany::one(UserContent::image_base64(
                    base64_data.to_string(),
                    Some(image_type),
                    Some(ImageDetail::default()),
                ));
                content_items.push(UserContent::text(
                    "Write a detailed caption for this image. Describe what you see, including any text, objects, people, and the overall scene.",
                ));

                let message = Message::User { content: content_items };

                agent.prompt(message).await
                    .map(|r| r.to_string())
                    .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
            }
            LlmProvider::OpenAI => {
                let client = openai::Client::from_env();
                let agent = client
                    .agent(&self.config.model)
                    .preamble(&self.config.preamble)
                    .temperature(self.config.temperature)
                    .build();

                let mut content_items = OneOrMany::one(UserContent::image_base64(
                    base64_data.to_string(),
                    Some(image_type),
                    Some(ImageDetail::default()),
                ));
                content_items.push(UserContent::text(
                    "Write a detailed caption for this image. Describe what you see, including any text, objects, people, and the overall scene.",
                ));

                let message = Message::User { content: content_items };

                agent.prompt(message).await
                    .map(|r| r.to_string())
                    .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
            }
            LlmProvider::DeepSeek => {
                let client = deepseek::Client::from_env();
                let agent = client
                    .agent(&self.config.model)
                    .preamble(&self.config.preamble)
                    .temperature(self.config.temperature)
                    .build();

                // DeepSeek may not support images, fall back to text prompt
                agent.prompt("Describe an image for me (note: DeepSeek doesn't support direct image input)").await
                    .map(|r| r.to_string())
                    .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
            }
        }
    }

    async fn complete(&self, prompt: &str) -> Result<String, MarkitdownError> {
        match self.config.provider {
            LlmProvider::Gemini => {
                let client = gemini::Client::from_env();
                let agent = client
                    .agent(&self.config.model)
                    .preamble(&self.config.preamble)
                    .temperature(self.config.temperature)
                    .build();

                agent.prompt(prompt).await
                    .map(|r| r.to_string())
                    .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
            }
            LlmProvider::OpenAI => {
                let client = openai::Client::from_env();
                let agent = client
                    .agent(&self.config.model)
                    .preamble(&self.config.preamble)
                    .temperature(self.config.temperature)
                    .build();

                agent.prompt(prompt).await
                    .map(|r| r.to_string())
                    .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
            }
            LlmProvider::DeepSeek => {
                let client = deepseek::Client::from_env();
                let agent = client
                    .agent(&self.config.model)
                    .preamble(&self.config.preamble)
                    .temperature(self.config.temperature)
                    .build();

                agent.prompt(prompt).await
                    .map(|r| r.to_string())
                    .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
            }
        }
    }
}

/// A mock LLM client for testing
pub struct MockLlmClient {
    pub image_response: String,
    pub text_response: String,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            image_response: "A mock image description".to_string(),
            text_response: "A mock text response".to_string(),
        }
    }

    pub fn with_image_response(mut self, response: impl Into<String>) -> Self {
        self.image_response = response.into();
        self
    }

    pub fn with_text_response(mut self, response: impl Into<String>) -> Self {
        self.text_response = response.into();
        self
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn describe_image(&self, _image_data: &[u8], _mime_type: &str) -> Result<String, MarkitdownError> {
        Ok(self.image_response.clone())
    }

    async fn describe_image_base64(&self, _base64_data: &str, _mime_type: &str) -> Result<String, MarkitdownError> {
        Ok(self.image_response.clone())
    }

    async fn complete(&self, _prompt: &str) -> Result<String, MarkitdownError> {
        Ok(self.text_response.clone())
    }
}

/// Create an LLM client from environment variables
/// Expects MARKITDOWN_LLM_PROVIDER and MARKITDOWN_LLM_MODEL, or falls back to
/// checking for API keys in the environment
pub fn create_llm_client_from_env() -> Option<Arc<dyn LlmClient>> {
    // Check for explicit configuration
    if let (Ok(provider), Ok(model)) = (
        std::env::var("MARKITDOWN_LLM_PROVIDER"),
        std::env::var("MARKITDOWN_LLM_MODEL"),
    ) {
        if let Ok(client) = GenericLlmClient::from_strings(&provider, &model) {
            return Some(Arc::new(client));
        }
    }

    // Auto-detect based on available API keys
    if std::env::var("GEMINI_API_KEY").is_ok() {
        return Some(Arc::new(GenericLlmClient::new(LlmConfig::gemini())));
    }

    if std::env::var("OPENAI_API_KEY").is_ok() {
        return Some(Arc::new(GenericLlmClient::new(LlmConfig::openai())));
    }

    if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        return Some(Arc::new(GenericLlmClient::new(LlmConfig::deepseek())));
    }

    None
}

// Legacy function for backward compatibility
pub async fn get_llm_description(
    image_data: &[u8],
    llm_client: &str,
    llm_model: &str,
) -> Option<String> {
    match GenericLlmClient::from_strings(llm_client, llm_model) {
        Ok(client) => client.describe_image(image_data, "image/jpeg").await.ok(),
        Err(_) => None,
    }
}
