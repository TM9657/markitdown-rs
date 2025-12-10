//! LLM abstraction for document conversion using any rig-core compatible model.
//!
//! This module provides a provider-agnostic LLM wrapper that works with ANY model
//! implementing rig-core's `CompletionModel` trait. This includes:
//! - OpenAI (GPT-4, GPT-4o, etc.)
//! - Anthropic (Claude)
//! - Google (Gemini)
//! - Cohere
//! - Any custom implementation
//!
//! # Example
//! ```ignore
//! use rig::providers::openai;
//! use markitdown::llm::{LlmWrapper, LlmConfig};
//!
//! let client = openai::Client::from_env();
//! let model = client.completion_model("gpt-4o");
//! let llm = LlmWrapper::new(model);
//! ```

use async_trait::async_trait;
use base64::prelude::*;
use futures::future::join_all;
use rig::{
    completion::{AssistantContent, CompletionModel, CompletionRequest, CompletionRequestBuilder},
    message::{ImageDetail, ImageMediaType, Message, UserContent},
    OneOrMany,
};
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::prompts::{
    DEFAULT_BATCH_IMAGE_PROMPT, DEFAULT_IMAGE_DESCRIPTION_PROMPT, DEFAULT_PAGE_CONVERSION_PROMPT,
};

/// Configuration for LLM behavior during document conversion.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// System prompt for single image description
    pub image_description_prompt: String,
    /// System prompt for full page conversion (PDF pages rendered as images)
    pub page_conversion_prompt: String,
    /// System prompt for batch image description (when images_per_message > 1)
    pub batch_image_prompt: String,
    /// Temperature for LLM responses (0.0 = deterministic, 1.0 = creative)
    pub temperature: f64,
    /// Number of images to send per LLM message for parallelization
    /// Default: 1 (one image per request)
    pub images_per_message: usize,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            image_description_prompt: DEFAULT_IMAGE_DESCRIPTION_PROMPT.to_string(),
            page_conversion_prompt: DEFAULT_PAGE_CONVERSION_PROMPT.to_string(),
            batch_image_prompt: DEFAULT_BATCH_IMAGE_PROMPT.to_string(),
            temperature: 0.3,
            images_per_message: 1,
        }
    }
}

impl LlmConfig {
    /// Create a new config with default prompts
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the image description prompt
    pub fn with_image_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.image_description_prompt = prompt.into();
        self
    }

    /// Set the page conversion prompt
    pub fn with_page_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.page_conversion_prompt = prompt.into();
        self
    }

    /// Set the batch image prompt
    pub fn with_batch_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.batch_image_prompt = prompt.into();
        self
    }

    /// Set the temperature
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.temperature = temp.clamp(0.0, 2.0);
        self
    }

    /// Set images per message for batching
    pub fn with_images_per_message(mut self, count: usize) -> Self {
        self.images_per_message = count.max(1);
        self
    }
}

/// Trait for LLM clients that can describe images and convert pages.
///
/// This trait is implemented for any type that wraps a rig-core CompletionModel,
/// allowing provider-agnostic LLM usage.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate a description for an image from raw bytes
    async fn describe_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<String, MarkitdownError>;

    /// Generate a description for an image from base64
    async fn describe_image_base64(
        &self,
        base64_data: &str,
        mime_type: &str,
    ) -> Result<String, MarkitdownError>;

    /// Generate descriptions for multiple images in batch
    /// Returns one description per image in the same order
    async fn describe_images_batch(
        &self,
        images: &[(&[u8], &str)], // (data, mime_type) pairs
    ) -> Result<Vec<String>, MarkitdownError>;

    /// Convert a page image to markdown (for PDF/scanned document conversion)
    async fn convert_page_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<String, MarkitdownError>;

    /// Generate a text completion
    async fn complete(&self, prompt: &str) -> Result<String, MarkitdownError>;

    /// Get the current configuration
    fn config(&self) -> &LlmConfig;

    /// Get number of images per message setting
    fn images_per_message(&self) -> usize {
        self.config().images_per_message
    }
}

/// Universal LLM wrapper that works with any rig-core CompletionModel.
///
/// This wrapper accepts any model implementing `CompletionModel`, making it
/// compatible with all rig-core providers (OpenAI, Anthropic, Google, etc.)
///
/// # Example
/// ```ignore
/// use rig::providers::openai;
/// use markitdown::llm::{LlmWrapper, LlmConfig};
///
/// // Create with any provider
/// let client = openai::Client::from_env();
/// let model = client.completion_model("gpt-4o");
/// let llm = LlmWrapper::new(model);
///
/// // Or with custom config
/// let llm = LlmWrapper::with_config(
///     model,
///     LlmConfig::default().with_images_per_message(3)
/// );
/// ```
pub struct LlmWrapper<M: CompletionModel> {
    model: Arc<M>,
    config: LlmConfig,
}

impl<M: CompletionModel> LlmWrapper<M> {
    /// Create a new LLM wrapper with default configuration
    pub fn new(model: M) -> Self {
        Self {
            model: Arc::new(model),
            config: LlmConfig::default(),
        }
    }

    /// Create a new LLM wrapper with custom configuration
    pub fn with_config(model: M, config: LlmConfig) -> Self {
        Self {
            model: Arc::new(model),
            config,
        }
    }

    /// Create from an Arc'd model
    pub fn from_arc(model: Arc<M>, config: LlmConfig) -> Self {
        Self { model, config }
    }

    /// Get a reference to the underlying model
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Get a mutable reference to the config
    pub fn config_mut(&mut self) -> &mut LlmConfig {
        &mut self.config
    }

    /// Build a completion request with the given system prompt and user content
    fn build_request(
        &self,
        system_prompt: &str,
        user_content: OneOrMany<UserContent>,
    ) -> CompletionRequestBuilder<M> {
        self.model
            .completion_request(system_prompt)
            .messages(vec![Message::User {
                content: user_content,
            }])
            .temperature(self.config.temperature)
    }

    /// Send a request and extract the response text
    async fn send_request(&self, request: CompletionRequest) -> Result<String, MarkitdownError> {
        self.model
            .completion(request)
            .await
            .map(|r| extract_text_from_response(&r.choice))
            .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))
    }
}

/// Extract text content from assistant response
fn extract_text_from_response(content: &OneOrMany<AssistantContent>) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            AssistantContent::Text(text) => Some(text.text.clone()),
            AssistantContent::Reasoning(r) => Some(r.reasoning.join("\n")),
            AssistantContent::ToolCall(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse MIME type string to rig ImageMediaType
fn parse_mime_to_image_type(mime_type: &str) -> ImageMediaType {
    match mime_type.to_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ImageMediaType::JPEG,
        "image/png" => ImageMediaType::PNG,
        "image/gif" => ImageMediaType::GIF,
        "image/webp" => ImageMediaType::WEBP,
        _ => ImageMediaType::PNG, // Default to PNG for unknown types
    }
}

#[async_trait]
impl<M: CompletionModel + Send + Sync + 'static> LlmClient for LlmWrapper<M> {
    async fn describe_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<String, MarkitdownError> {
        let base64_data = BASE64_STANDARD.encode(image_data);
        self.describe_image_base64(&base64_data, mime_type).await
    }

    async fn describe_image_base64(
        &self,
        base64_data: &str,
        mime_type: &str,
    ) -> Result<String, MarkitdownError> {
        let image_type = parse_mime_to_image_type(mime_type);

        let mut content = OneOrMany::one(UserContent::image_base64(
            base64_data.to_string(),
            Some(image_type),
            Some(ImageDetail::Auto),
        ));
        content.push(UserContent::text("Describe this image in detail."));

        let request = self
            .build_request(&self.config.image_description_prompt, content)
            .build();

        self.send_request(request).await
    }

    async fn describe_images_batch(
        &self,
        images: &[(&[u8], &str)],
    ) -> Result<Vec<String>, MarkitdownError> {
        if images.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = self.config.images_per_message;

        if batch_size == 1 {
            // Process one at a time in parallel
            let futures: Vec<_> = images
                .iter()
                .map(|(data, mime)| self.describe_image(*data, *mime))
                .collect();

            let results = join_all(futures).await;
            results.into_iter().collect()
        } else {
            // Process in batches
            let mut all_descriptions = Vec::with_capacity(images.len());

            for chunk in images.chunks(batch_size) {
                if chunk.len() == 1 {
                    // Single image, use regular method
                    let desc = self.describe_image(chunk[0].0, chunk[0].1).await?;
                    all_descriptions.push(desc);
                } else {
                    // Multiple images in one request
                    let mut content =
                        OneOrMany::one(UserContent::text(&self.config.batch_image_prompt));

                    for (i, (data, mime)) in chunk.iter().enumerate() {
                        let base64_data = BASE64_STANDARD.encode(data);
                        let image_type = parse_mime_to_image_type(mime);
                        content.push(UserContent::text(format!("\n--- Image {} ---", i + 1)));
                        content.push(UserContent::image_base64(
                            base64_data,
                            Some(image_type),
                            Some(ImageDetail::Auto),
                        ));
                    }

                    let request = self
                        .build_request(&self.config.batch_image_prompt, content)
                        .build();

                    let response = self.send_request(request).await?;

                    // Parse the response to extract individual descriptions
                    let descriptions = parse_batch_response(&response, chunk.len());
                    all_descriptions.extend(descriptions);
                }
            }

            Ok(all_descriptions)
        }
    }

    async fn convert_page_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<String, MarkitdownError> {
        let base64_data = BASE64_STANDARD.encode(image_data);
        let image_type = parse_mime_to_image_type(mime_type);

        let mut content = OneOrMany::one(UserContent::image_base64(
            base64_data,
            Some(image_type),
            Some(ImageDetail::High), // Use high detail for page conversion
        ));
        content.push(UserContent::text(
            "Convert this document page to clean, well-structured markdown.",
        ));

        let request = self
            .build_request(&self.config.page_conversion_prompt, content)
            .build();

        self.send_request(request).await
    }

    async fn complete(&self, prompt: &str) -> Result<String, MarkitdownError> {
        let content = OneOrMany::one(UserContent::text(prompt));
        let request = self
            .build_request("You are a helpful assistant.", content)
            .build();

        self.send_request(request).await
    }

    fn config(&self) -> &LlmConfig {
        &self.config
    }
}

/// Parse a batch response to extract individual image descriptions
fn parse_batch_response(response: &str, expected_count: usize) -> Vec<String> {
    // Try to split by "## Image N" headers
    let mut descriptions = Vec::with_capacity(expected_count);
    let parts: Vec<&str> = response.split("## Image").collect();

    if parts.len() > 1 {
        // Found headers, extract content after each
        for part in parts.iter().skip(1) {
            // Skip the number and extract content
            let content = part
                .trim()
                .splitn(2, '\n')
                .nth(1)
                .unwrap_or(part.trim())
                .trim();
            descriptions.push(content.to_string());
        }
    }

    // If we didn't find enough descriptions, try splitting by "---"
    if descriptions.len() < expected_count {
        descriptions.clear();
        let parts: Vec<&str> = response.split("---").collect();
        for part in &parts {
            let trimmed = part.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("Image") {
                descriptions.push(trimmed.to_string());
            }
        }
    }

    // If still not enough, just return the whole response for each
    if descriptions.len() < expected_count {
        descriptions = vec![response.to_string(); expected_count];
    } else if descriptions.len() > expected_count {
        descriptions.truncate(expected_count);
    }

    descriptions
}

/// A mock LLM client for testing
pub struct MockLlmClient {
    pub image_response: String,
    pub text_response: String,
    config: LlmConfig,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            image_response: "A mock image description".to_string(),
            text_response: "A mock text response".to_string(),
            config: LlmConfig::default(),
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

    pub fn with_config(mut self, config: LlmConfig) -> Self {
        self.config = config;
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
    async fn describe_image(
        &self,
        _image_data: &[u8],
        _mime_type: &str,
    ) -> Result<String, MarkitdownError> {
        Ok(self.image_response.clone())
    }

    async fn describe_image_base64(
        &self,
        _base64_data: &str,
        _mime_type: &str,
    ) -> Result<String, MarkitdownError> {
        Ok(self.image_response.clone())
    }

    async fn describe_images_batch(
        &self,
        images: &[(&[u8], &str)],
    ) -> Result<Vec<String>, MarkitdownError> {
        Ok(vec![self.image_response.clone(); images.len()])
    }

    async fn convert_page_image(
        &self,
        _image_data: &[u8],
        _mime_type: &str,
    ) -> Result<String, MarkitdownError> {
        Ok(self.text_response.clone())
    }

    async fn complete(&self, _prompt: &str) -> Result<String, MarkitdownError> {
        Ok(self.text_response.clone())
    }

    fn config(&self) -> &LlmConfig {
        &self.config
    }
}

/// Type alias for a boxed LLM client that can be shared across threads
pub type SharedLlmClient = Arc<dyn LlmClient>;

/// Helper function to create a shared LLM client from any CompletionModel
pub fn create_llm_client<M: CompletionModel + Send + Sync + 'static>(model: M) -> SharedLlmClient {
    Arc::new(LlmWrapper::new(model))
}

/// Helper function to create a shared LLM client with custom config
pub fn create_llm_client_with_config<M: CompletionModel + Send + Sync + 'static>(
    model: M,
    config: LlmConfig,
) -> SharedLlmClient {
    Arc::new(LlmWrapper::with_config(model, config))
}
