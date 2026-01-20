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
use crate::model::ExtractedImage;
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
    /// Maximum tokens for LLM output (None = no limit)
    /// Helps prevent runaway generation from poorly-behaved models
    pub max_tokens: Option<u64>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            image_description_prompt: DEFAULT_IMAGE_DESCRIPTION_PROMPT.to_string(),
            page_conversion_prompt: DEFAULT_PAGE_CONVERSION_PROMPT.to_string(),
            batch_image_prompt: DEFAULT_BATCH_IMAGE_PROMPT.to_string(),
            temperature: 0.1, // Low temperature for accurate extraction with minimal repetition
            images_per_message: 1,
            max_tokens: Some(4096), // Reasonable default to prevent runaway generation
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

    /// Set maximum tokens for LLM output
    /// Use None for no limit (not recommended for production)
    pub fn with_max_tokens(mut self, tokens: Option<u64>) -> Self {
        self.max_tokens = tokens;
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

    /// Generate descriptions for multiple ExtractedImage references in batch.
    /// This is the preferred method as it provides access to image metadata
    /// (alt_text, dimensions, page_number) for better context-aware descriptions.
    /// Returns one description per image in the same order.
    async fn describe_extracted_images(
        &self,
        images: &[&ExtractedImage],
    ) -> Result<Vec<String>, MarkitdownError> {
        // Default implementation delegates to describe_images_batch
        let data: Vec<(&[u8], &str)> = images
            .iter()
            .map(|img| (img.data.as_ref(), img.mime_type.as_str()))
            .collect();
        self.describe_images_batch(&data).await
    }

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
        let mut builder = self
            .model
            .completion_request(system_prompt)
            .messages(vec![Message::User {
                content: user_content,
            }])
            .temperature(self.config.temperature);

        if let Some(max_tokens) = self.config.max_tokens {
            builder = builder.max_tokens(max_tokens);
        }

        builder
    }

    /// Send a request and extract the response text, with post-processing for repetition detection
    async fn send_request(&self, request: CompletionRequest) -> Result<String, MarkitdownError> {
        let response = self
            .model
            .completion(request)
            .await
            .map(|r| extract_text_from_response(&r.choice))
            .map_err(|e| MarkitdownError::LlmError(format!("LLM error: {}", e)))?;

        // Apply repetition detection and cleanup
        Ok(detect_and_truncate_repetition(&response))
    }

    /// Describe a single ExtractedImage with context-aware prompts
    async fn describe_single_extracted_image(
        &self,
        img: &ExtractedImage,
    ) -> Result<String, MarkitdownError>
    where
        M: Send + Sync + 'static,
    {
        let base64_data = img.to_base64();
        let image_type = parse_mime_to_image_type(&img.mime_type);

        let mut content = OneOrMany::one(UserContent::image_base64(
            base64_data,
            Some(image_type),
            Some(ImageDetail::Auto),
        ));

        // Build a context-aware prompt
        let mut prompt = String::from("Describe this image in detail.");
        if let Some(alt) = &img.alt_text {
            prompt.push_str(&format!(
                " The existing alt text is: '{}'. Build upon or improve this description.",
                alt
            ));
        }
        if let (Some(w), Some(h)) = (img.width, img.height) {
            prompt.push_str(&format!(" The image is {}x{} pixels.", w, h));
        }
        if let Some(page) = img.page_number {
            prompt.push_str(&format!(
                " This image appears on page {} of the document.",
                page
            ));
        }
        if let Some(path) = &img.source_path {
            prompt.push_str(&format!(
                " The source path is: '{}'. Use this as a contextual hint if relevant.",
                path
            ));
        }

        content.push(UserContent::text(prompt));

        let request = self
            .build_request(&self.config.image_description_prompt, content)
            .build();

        self.send_request(request).await
    }

    async fn describe_extracted_images_individual(
        &self,
        images: &[&ExtractedImage],
    ) -> Result<Vec<String>, MarkitdownError>
    where
        M: Send + Sync + 'static,
    {
        let futures: Vec<_> = images
            .iter()
            .map(|img| self.describe_single_extracted_image(img))
            .collect();

        let results = join_all(futures).await;
        results.into_iter().collect()
    }

    async fn describe_extracted_images_batched(
        &self,
        images: &[&ExtractedImage],
    ) -> Result<Vec<String>, MarkitdownError>
    where
        M: Send + Sync + 'static,
    {
        let mut all_descriptions = Vec::with_capacity(images.len());
        let batch_size = self.config.images_per_message;

        for chunk in images.chunks(batch_size) {
            if chunk.len() == 1 {
                let desc = self.describe_single_extracted_image(chunk[0]).await?;
                all_descriptions.push(desc);
                continue;
            }

            let mut content = OneOrMany::one(UserContent::text(&self.config.batch_image_prompt));

            for (i, img) in chunk.iter().enumerate() {
                let base64_data = img.to_base64();
                let image_type = parse_mime_to_image_type(&img.mime_type);

                let mut context = format!("\n--- Image {} ---", i + 1);
                if let Some(alt) = &img.alt_text {
                    context.push_str(&format!("\nExisting alt text: {}", alt));
                }
                if let (Some(w), Some(h)) = (img.width, img.height) {
                    context.push_str(&format!("\nDimensions: {}x{} pixels", w, h));
                }
                if let Some(page) = img.page_number {
                    context.push_str(&format!("\nFrom page: {}", page));
                }
                if let Some(path) = &img.source_path {
                    context.push_str(&format!("\nSource path hint: {}", path));
                }

                content.push(UserContent::text(context));
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
            let descriptions = parse_batch_response(&response, chunk.len());
            all_descriptions.extend(descriptions);
        }

        Ok(all_descriptions)
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

/// Detect and truncate repetitive content from LLM output.
///
/// Some models (especially free tier ones) can get stuck in loops, generating
/// the same phrase thousands of times. This function detects such patterns
/// and truncates the output at the first repetition.
fn detect_and_truncate_repetition(text: &str) -> String {
    // If text is short, no need to check
    if text.len() < 500 {
        return text.to_string();
    }

    // Helper to find nearest char boundary at or before position
    fn find_char_boundary(s: &str, pos: usize) -> usize {
        if pos >= s.len() {
            return s.len();
        }
        let mut idx = pos;
        while idx > 0 && !s.is_char_boundary(idx) {
            idx -= 1;
        }
        idx
    }

    // Strategy 1: Check for repeating substrings using sliding window
    // Look for any 30+ char pattern that repeats 3+ times
    let check_sizes = [30, 50, 80, 100];
    for &window_size in &check_sizes {
        if text.len() > window_size * 4 {
            // Sample multiple positions throughout the text
            let sample_count = 10.min(text.len() / window_size);
            for i in 0..sample_count {
                let raw_start = (text.len() / sample_count) * i;
                let start = find_char_boundary(text, raw_start);
                let raw_end = start + window_size;
                let end = find_char_boundary(text, raw_end);

                if end <= text.len() && start < end {
                    let sample = &text[start..end];
                    // Skip if sample is mostly whitespace
                    if sample.chars().filter(|c| !c.is_whitespace()).count() < sample.len() / 3 {
                        continue;
                    }
                    let count = text.matches(sample).count();
                    if count >= 3 {
                        // Found repetition - truncate at first occurrence + buffer
                        if let Some(first_pos) = text.find(sample) {
                            let raw_end_pos = first_pos + sample.len() * 2;
                            let end_pos = find_char_boundary(text, raw_end_pos.min(text.len()));
                            return format!(
                                "{}\n\n[Note: Repetitive content detected and truncated]",
                                &text[..end_pos]
                            );
                        }
                    }
                }
            }
        }
    }

    // Strategy 2: Look for consecutive identical lines (fallback)
    let lines: Vec<&str> = text.lines().collect();
    let mut seen_consecutive = 0;
    let mut last_line = "";
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() < 10 {
            continue;
        }
        if trimmed == last_line {
            seen_consecutive += 1;
            if seen_consecutive >= 4 {
                let truncate_at = i - seen_consecutive;
                let truncated: Vec<&str> = lines[..truncate_at].to_vec();
                return format!(
                    "{}\n\n[Note: Repetitive content detected and truncated]",
                    truncated.join("\n")
                );
            }
        } else {
            seen_consecutive = 1;
            last_line = trimmed;
        }
    }

    // Strategy 3: Check if output is suspiciously long for a single page
    // Normal pages are typically 500-3000 words; anything over 5000 is suspicious
    let word_count = text.split_whitespace().count();
    if word_count > 5000 {
        // Truncate to approximately 3000 words
        let words: Vec<&str> = text.split_whitespace().take(3000).collect();
        let truncated = words.join(" ");
        return format!(
            "{}\n\n[Note: Output truncated due to excessive length - possible repetition]",
            truncated
        );
    }

    text.to_string()
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
            "Convert this page to markdown. Output only the content, no commentary.",
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

    async fn describe_extracted_images(
        &self,
        images: &[&ExtractedImage],
    ) -> Result<Vec<String>, MarkitdownError> {
        if images.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = self.config.images_per_message;

        if batch_size == 1 {
            self.describe_extracted_images_individual(images).await
        } else {
            self.describe_extracted_images_batched(images).await
        }
    }
}

/// Parse a batch response to extract individual image descriptions
fn parse_batch_response(response: &str, expected_count: usize) -> Vec<String> {
    let descriptions = parse_batch_response_with_headers(response, expected_count)
        .or_else(|| parse_batch_response_with_separators(response, expected_count))
        .unwrap_or_else(|| vec![response.to_string(); expected_count]);

    normalize_batch_descriptions(descriptions, response, expected_count)
}

fn parse_batch_response_with_headers(response: &str, expected_count: usize) -> Option<Vec<String>> {
    let parts: Vec<&str> = response.split("## Image").collect();
    if parts.len() <= 1 {
        return None;
    }

    let mut descriptions = Vec::with_capacity(expected_count);
    for part in parts.iter().skip(1) {
        let content = part
            .trim()
            .splitn(2, '\n')
            .nth(1)
            .unwrap_or(part.trim())
            .trim();
        if !content.is_empty() {
            descriptions.push(content.to_string());
        }
    }

    if descriptions.is_empty() {
        None
    } else {
        Some(descriptions)
    }
}

fn parse_batch_response_with_separators(
    response: &str,
    expected_count: usize,
) -> Option<Vec<String>> {
    let mut descriptions = Vec::with_capacity(expected_count);
    for part in response.split("---") {
        let trimmed = part.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("Image") {
            descriptions.push(trimmed.to_string());
        }
    }

    if descriptions.is_empty() {
        None
    } else {
        Some(descriptions)
    }
}

fn normalize_batch_descriptions(
    mut descriptions: Vec<String>,
    response: &str,
    expected_count: usize,
) -> Vec<String> {
    if descriptions.len() < expected_count {
        return vec![response.to_string(); expected_count];
    }

    if descriptions.len() > expected_count {
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
