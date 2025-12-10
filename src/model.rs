use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::llm::{LlmClient, SharedLlmClient};

/// Represents an extracted image from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedImage {
    /// Unique identifier for this image within the document
    pub id: String,
    /// Image data as bytes (raw image data, e.g., PNG, JPEG)
    #[serde(skip_serializing, skip_deserializing)]
    pub data: Bytes,
    /// MIME type of the image (e.g., "image/png", "image/jpeg")
    pub mime_type: String,
    /// Optional alt text or caption if available from source
    pub alt_text: Option<String>,
    /// Optional LLM-generated description
    pub description: Option<String>,
    /// Width in pixels if known
    pub width: Option<u32>,
    /// Height in pixels if known
    pub height: Option<u32>,
    /// Page number where the image appears (1-indexed)
    pub page_number: Option<u32>,
}

impl ExtractedImage {
    pub fn new(id: impl Into<String>, data: Bytes, mime_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            data,
            mime_type: mime_type.into(),
            alt_text: None,
            description: None,
            width: None,
            height: None,
            page_number: None,
        }
    }

    /// Get base64 encoded image data
    pub fn to_base64(&self) -> String {
        use base64::prelude::*;
        BASE64_STANDARD.encode(&self.data)
    }

    /// Get a markdown image reference (placeholder for replacement)
    pub fn as_markdown_placeholder(&self) -> String {
        format!("![Image: {}](image:{})", self.id, self.id)
    }

    /// Get the description or fallback to alt_text
    pub fn get_display_text(&self) -> Option<&str> {
        self.description.as_deref().or(self.alt_text.as_deref())
    }
}

/// Represents a block of content in a page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    /// Plain text content
    Text(String),
    /// A heading with level (1-6) and text
    Heading { level: u8, text: String },
    /// An image reference
    Image(ExtractedImage),
    /// A table with headers and rows
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// A list (ordered or unordered)
    List { ordered: bool, items: Vec<String> },
    /// A code block with optional language
    Code {
        language: Option<String>,
        code: String,
    },
    /// A blockquote
    Quote(String),
    /// Raw markdown (already formatted)
    Markdown(String),
}

impl ContentBlock {
    /// Convert this content block to markdown
    pub fn to_markdown(&self) -> String {
        match self {
            ContentBlock::Text(text) => format!("{}\n", text),
            ContentBlock::Heading { level, text } => {
                format!("{} {}\n", "#".repeat(*level as usize), text)
            }
            ContentBlock::Image(img) => {
                if let Some(desc) = img.get_display_text() {
                    format!("![{}]({})\n\n*{}*\n", img.id, img.id, desc)
                } else {
                    format!("![{}]({})\n", img.id, img.id)
                }
            }
            ContentBlock::Table { headers, rows } => {
                let mut md = String::new();
                md.push_str("| ");
                md.push_str(&headers.join(" | "));
                md.push_str(" |\n| ");
                md.push_str(
                    &headers
                        .iter()
                        .map(|_| "---")
                        .collect::<Vec<_>>()
                        .join(" | "),
                );
                md.push_str(" |\n");
                for row in rows {
                    md.push_str("| ");
                    md.push_str(&row.join(" | "));
                    md.push_str(" |\n");
                }
                md
            }
            ContentBlock::List { ordered, items } => {
                let mut md = String::new();
                for (i, item) in items.iter().enumerate() {
                    if *ordered {
                        md.push_str(&format!("{}. {}\n", i + 1, item));
                    } else {
                        md.push_str(&format!("- {}\n", item));
                    }
                }
                md
            }
            ContentBlock::Code { language, code } => {
                format!("```{}\n{}\n```\n", language.as_deref().unwrap_or(""), code)
            }
            ContentBlock::Quote(text) => {
                text.lines()
                    .map(|line| format!("> {}", line))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            }
            ContentBlock::Markdown(md) => md.clone(),
        }
    }
}

/// Represents a single page of content extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Page number (1-indexed)
    pub page_number: u32,
    /// Content blocks in order
    pub content: Vec<ContentBlock>,
    /// Optional rendered image of the entire page (for scanned PDFs, slides, complex layouts)
    pub rendered_image: Option<ExtractedImage>,
}

impl Page {
    pub fn new(page_number: u32) -> Self {
        Self {
            page_number,
            content: Vec::new(),
            rendered_image: None,
        }
    }

    /// Set the rendered image for this page
    pub fn with_rendered_image(mut self, image: ExtractedImage) -> Self {
        self.rendered_image = Some(image);
        self
    }

    /// Add a content block to the page
    pub fn add_content(&mut self, block: ContentBlock) {
        self.content.push(block);
    }

    /// Get all images from this page
    pub fn images(&self) -> Vec<&ExtractedImage> {
        self.content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Image(img) = block {
                    Some(img)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all text content (excluding images) as markdown
    pub fn to_markdown(&self) -> String {
        self.content
            .iter()
            .map(|block| block.to_markdown())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Create a new page with images replaced by their LLM descriptions
    /// Uses batch processing based on the LLM client's images_per_message setting
    pub async fn with_image_descriptions(
        &self,
        llm_client: &dyn LlmClient,
    ) -> Result<Page, MarkitdownError> {
        // Collect images that need descriptions
        let images_to_describe: Vec<(usize, &ExtractedImage)> = self
            .content
            .iter()
            .enumerate()
            .filter_map(|(i, block)| {
                if let ContentBlock::Image(img) = block {
                    if img.description.is_none() {
                        return Some((i, img));
                    }
                }
                None
            })
            .collect();

        // If no images need descriptions, return a clone
        if images_to_describe.is_empty() {
            return Ok(self.clone());
        }

        // Prepare batch data
        let image_data: Vec<(&[u8], &str)> = images_to_describe
            .iter()
            .map(|(_, img)| (img.data.as_ref(), img.mime_type.as_str()))
            .collect();

        // Get descriptions in batch
        let descriptions = llm_client.describe_images_batch(&image_data).await?;

        // Build the new page with descriptions
        let mut new_page = Page::new(self.page_number);
        let mut desc_iter = descriptions.into_iter();
        let mut image_indices: std::collections::HashSet<usize> =
            images_to_describe.iter().map(|(i, _)| *i).collect();

        for (i, block) in self.content.iter().enumerate() {
            match block {
                ContentBlock::Image(img) if image_indices.remove(&i) => {
                    let mut new_img = img.clone();
                    if let Some(desc) = desc_iter.next() {
                        new_img.description = Some(desc);
                    }
                    new_page.add_content(ContentBlock::Image(new_img));
                }
                other => new_page.add_content(other.clone()),
            }
        }

        Ok(new_page)
    }

    /// Convert this page to a text-only page, replacing images with their descriptions
    pub fn to_text_only(&self) -> Page {
        let mut new_page = Page::new(self.page_number);

        for block in &self.content {
            match block {
                ContentBlock::Image(img) => {
                    if let Some(text) = img.get_display_text() {
                        new_page.add_content(ContentBlock::Text(format!("[Image: {}]", text)));
                    } else {
                        new_page.add_content(ContentBlock::Text(format!("[Image: {}]", img.id)));
                    }
                }
                other => new_page.add_content(other.clone()),
            }
        }

        new_page
    }
}

/// Represents a complete document with multiple pages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Optional document title
    pub title: Option<String>,
    /// Document pages
    pub pages: Vec<Page>,
    /// Document-level metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            title: None,
            pages: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a document from a single page
    pub fn from_page(page: Page) -> Self {
        Self {
            title: None,
            pages: vec![page],
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a page to the document
    pub fn add_page(&mut self, page: Page) {
        self.pages.push(page);
    }

    /// Get all images from the document
    pub fn images(&self) -> Vec<&ExtractedImage> {
        self.pages.iter().flat_map(|p| p.images()).collect()
    }

    /// Convert the entire document to markdown
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        if let Some(title) = &self.title {
            md.push_str(&format!("# {}\n\n", title));
        }

        for page in &self.pages {
            if self.pages.len() > 1 {
                md.push_str(&format!("\n---\n## Page {}\n\n", page.page_number));
            }
            md.push_str(&page.to_markdown());
        }

        md
    }

    /// Create a new document with all images replaced by LLM descriptions
    pub async fn with_image_descriptions(
        &self,
        llm_client: &dyn LlmClient,
    ) -> Result<Document, MarkitdownError> {
        let mut new_doc = Document::new();
        new_doc.title = self.title.clone();
        new_doc.metadata = self.metadata.clone();

        for page in &self.pages {
            new_doc.add_page(page.with_image_descriptions(llm_client).await?);
        }

        Ok(new_doc)
    }

    /// Convert to text-only document
    pub fn to_text_only(&self) -> Document {
        let mut new_doc = Document::new();
        new_doc.title = self.title.clone();
        new_doc.metadata = self.metadata.clone();

        for page in &self.pages {
            new_doc.add_page(page.to_text_only());
        }

        new_doc
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy result type for backward compatibility
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentConverterResult {
    pub title: Option<String>,
    pub text_content: String,
}

impl From<Document> for DocumentConverterResult {
    fn from(doc: Document) -> Self {
        Self {
            title: doc.title.clone(),
            text_content: doc.to_markdown(),
        }
    }
}

/// Options for document conversion
#[derive(Clone)]
pub struct ConversionOptions {
    /// File extension hint (e.g., ".pdf", ".docx")
    pub file_extension: Option<String>,
    /// Source URL if applicable
    pub url: Option<String>,
    /// Optional LLM client for image descriptions
    pub llm_client: Option<SharedLlmClient>,
    /// Whether to extract images
    pub extract_images: bool,
}

impl std::fmt::Debug for ConversionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversionOptions")
            .field("file_extension", &self.file_extension)
            .field("url", &self.url)
            .field(
                "llm_client",
                &self.llm_client.as_ref().map(|_| "<LlmClient>"),
            )
            .field("extract_images", &self.extract_images)
            .finish()
    }
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            file_extension: None,
            url: None,
            llm_client: None,
            extract_images: true,
        }
    }
}

impl ConversionOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_extension(mut self, ext: impl Into<String>) -> Self {
        self.file_extension = Some(ext.into());
        self
    }

    pub fn with_llm(mut self, client: SharedLlmClient) -> Self {
        self.llm_client = Some(client);
        self
    }

    pub fn with_images(mut self, extract: bool) -> Self {
        self.extract_images = extract;
        self
    }
}

/// Trait for document converters that work with ObjectStore
#[async_trait]
pub trait DocumentConverter: Send + Sync {
    /// Convert a file from ObjectStore to a Document
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError>;

    /// Convert bytes directly to a Document
    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError>;

    /// Get supported file extensions
    fn supported_extensions(&self) -> &[&str];

    /// Check if this converter can handle the given extension
    fn can_handle(&self, extension: &str) -> bool {
        let ext_normalized = extension.trim_start_matches('.');
        self.supported_extensions().iter().any(|supported| {
            supported
                .trim_start_matches('.')
                .eq_ignore_ascii_case(ext_normalized)
        })
    }
}
