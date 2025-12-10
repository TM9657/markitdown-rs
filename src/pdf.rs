//! PDF to Markdown converter with smart LLM fallback.
//!
//! This module provides PDF conversion with intelligent handling of:
//! - Regular PDFs with extractable text
//! - Scanned PDFs (rendered as images for LLM OCR)
//! - Complex pages with diagrams/charts (sent to LLM for description)
//! - Pages with embedded images + limited text (rendered for full context)
//!
//! Uses hayro for PDF rendering when LLM fallback is needed.

use async_trait::async_trait;
use bytes::Bytes;
use hayro::{render, InterpreterSettings, Pdf, RenderSettings};
use object_store::ObjectStore;
use pdf_extract;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::llm::LlmClient;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// Threshold for "low text" - if page has images and text below this, render whole page
const LOW_TEXT_WORD_THRESHOLD: usize = 350;

/// Metrics for a PDF page to determine if LLM processing is needed
#[derive(Debug)]
struct PageMetrics {
    /// Extracted text content
    text: String,
    /// Number of characters
    char_count: usize,
    /// Number of words
    word_count: usize,
    /// Ratio of alphanumeric characters to total
    alpha_ratio: f64,
    /// Whether the text seems structured (has line breaks, paragraphs)
    is_structured: bool,
    /// Number of XObjects (images/forms) on this page
    xobject_count: usize,
}

impl PageMetrics {
    fn from_text_with_xobjects(text: &str, xobject_count: usize) -> Self {
        let text = text.trim().to_string();
        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();

        let alpha_count = text.chars().filter(|c| c.is_alphanumeric()).count();
        let alpha_ratio = if char_count > 0 {
            alpha_count as f64 / char_count as f64
        } else {
            0.0
        };

        let line_count = text.lines().count();
        let is_structured = line_count > 3 && word_count > 10;

        Self {
            text,
            char_count,
            word_count,
            alpha_ratio,
            is_structured,
            xobject_count,
        }
    }

    /// Determine if this page should be processed by LLM
    fn should_use_llm(&self) -> bool {
        // Very little text = likely scanned
        if self.word_count < 10 {
            return true;
        }

        // Low alphanumeric ratio = likely garbage/OCR artifacts
        if self.alpha_ratio < 0.5 {
            return true;
        }

        // Very short unstructured content = might be missing context
        if self.char_count < 50 && !self.is_structured {
            return true;
        }

        // If we have to use LLM for images anyway, giving it the whole
        // page provides better context for conversion. But only if text is limited
        // to avoid OCR errors on text-heavy pages.
        if self.xobject_count > 0 && self.word_count < LOW_TEXT_WORD_THRESHOLD {
            return true;
        }

        false
    }
}

/// PDF converter with smart LLM fallback for scanned/complex pages.
pub struct PdfConverter;

impl PdfConverter {
    /// Render a PDF page as PNG image using hayro
    fn render_page_as_image(pdf: &Pdf, page_index: usize) -> Result<Vec<u8>, MarkitdownError> {
        let pages = pdf.pages();
        let page = pages.get(page_index).ok_or_else(|| {
            MarkitdownError::ParseError(format!("Page {} not found", page_index + 1))
        })?;

        let interpreter_settings = InterpreterSettings::default();
        let render_settings = RenderSettings::default();

        let pixmap = render(&page, &interpreter_settings, &render_settings);

        // Encode as PNG (take_png consumes the pixmap)
        let png_data = pixmap.take_png();

        Ok(png_data)
    }

    /// Parse PDF using hayro
    fn parse_pdf(bytes: &[u8]) -> Result<Pdf, MarkitdownError> {
        let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes.to_vec());
        Pdf::new(data)
            .map_err(|e| MarkitdownError::ParseError(format!("Failed to parse PDF: {:?}", e)))
    }

    /// Extract text from PDF and split by pages
    fn extract_text_by_page(bytes: &[u8]) -> Result<Vec<String>, MarkitdownError> {
        let text_content = pdf_extract::extract_text_from_mem(bytes).map_err(|e| {
            MarkitdownError::ParseError(format!("Failed to extract text from PDF: {}", e))
        })?;

        // Split by form feed (page separator)
        let pages: Vec<String> = text_content.split('\x0c').map(|s| s.to_string()).collect();

        Ok(pages)
    }

    /// Count XObjects (images, forms) on a page
    fn count_page_xobjects(pdf: &Pdf, page_index: usize) -> usize {
        let pages = pdf.pages();
        if let Some(page) = pages.get(page_index) {
            // XObjects dictionary contains Image and Form XObjects
            page.resources().x_objects.len()
        } else {
            0
        }
    }

    /// Convert PDF with optional LLM fallback for complex/scanned pages
    async fn convert_with_llm(
        &self,
        bytes: &[u8],
        llm_client: Option<&dyn LlmClient>,
    ) -> Result<Document, MarkitdownError> {
        let page_texts = Self::extract_text_by_page(bytes)?;
        let pdf = Self::parse_pdf(bytes).ok();
        let page_count = pdf
            .as_ref()
            .map(|p| p.pages().len())
            .unwrap_or(page_texts.len());

        let mut document = Document::new();

        for (idx, page_text) in page_texts.iter().enumerate() {
            // Count XObjects if we have the PDF parsed
            let xobject_count = pdf
                .as_ref()
                .map(|p| Self::count_page_xobjects(p, idx))
                .unwrap_or(0);

            let metrics = PageMetrics::from_text_with_xobjects(page_text, xobject_count);

            // Determine if we should use LLM for this page
            let use_llm = metrics.should_use_llm() && llm_client.is_some() && pdf.is_some();

            let mut page = Page::new((idx + 1) as u32);

            if use_llm {
                if let (Some(llm), Some(ref pdf_ref)) = (llm_client, &pdf) {
                    // Render page as image and send to LLM
                    match Self::render_page_as_image(pdf_ref, idx) {
                        Ok(png_data) => {
                            match llm.convert_page_image(&png_data, "image/png").await {
                                Ok(markdown) => {
                                    page.add_content(ContentBlock::Markdown(markdown));
                                }
                                Err(_) => {
                                    // LLM failed, fall back to extracted text
                                    if !metrics.text.is_empty() {
                                        page.add_content(ContentBlock::Text(metrics.text.clone()));
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            // Rendering failed, use extracted text
                            if !metrics.text.is_empty() {
                                page.add_content(ContentBlock::Text(metrics.text.clone()));
                            }
                        }
                    }
                }
            } else {
                // Use extracted text directly
                if !metrics.text.is_empty() {
                    page.add_content(ContentBlock::Text(metrics.text));
                }
            }

            // Only add page if it has content
            if !page.content.is_empty() {
                document.add_page(page);
            }
        }

        // Handle case where we have pages but no extracted text
        if document.pages.is_empty() && page_count > 0 {
            // Try rendering all pages with LLM if available
            if let (Some(llm), Some(ref pdf_ref)) = (llm_client, &pdf) {
                for idx in 0..page_count {
                    let mut page = Page::new((idx + 1) as u32);

                    if let Ok(png_data) = Self::render_page_as_image(pdf_ref, idx) {
                        if let Ok(markdown) = llm.convert_page_image(&png_data, "image/png").await {
                            page.add_content(ContentBlock::Markdown(markdown));
                        }
                    }

                    if !page.content.is_empty() {
                        document.add_page(page);
                    }
                }
            }
        }

        // Absolute fallback: create a document noting it couldn't be processed
        if document.pages.is_empty() {
            let mut page = Page::new(1);
            page.add_content(ContentBlock::Text(
                "[PDF content could not be extracted. The document may be scanned or protected.]"
                    .to_string(),
            ));
            document.add_page(page);
        }

        Ok(document)
    }

    /// Basic conversion without LLM (original behavior)
    fn convert_basic(&self, bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let text_content = pdf_extract::extract_text_from_mem(bytes).map_err(|e| {
            MarkitdownError::ParseError(format!("Failed to extract text from PDF: {}", e))
        })?;

        let mut document = Document::new();
        let pages: Vec<&str> = text_content.split('\x0c').collect();

        for (idx, page_text) in pages.iter().enumerate() {
            let trimmed = page_text.trim();
            if !trimmed.is_empty() {
                let mut page = Page::new((idx + 1) as u32);
                page.add_content(ContentBlock::Text(trimmed.to_string()));
                document.add_page(page);
            }
        }

        if document.pages.is_empty() {
            let mut page = Page::new(1);
            page.add_content(ContentBlock::Text(text_content));
            document.add_page(page);
        }

        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for PdfConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        if let Some(opts) = &options {
            if let Some(ext) = &opts.file_extension {
                if ext != ".pdf" {
                    return Err(MarkitdownError::InvalidFile(format!(
                        "Expected .pdf file, got {}",
                        ext
                    )));
                }
            }
        }

        let result = store.get(path).await?;
        let bytes = result.bytes().await?;

        // Check if we have an LLM client for smart conversion
        if let Some(opts) = &options {
            if let Some(llm) = &opts.llm_client {
                return self.convert_with_llm(&bytes, Some(llm.as_ref())).await;
            }
        }

        self.convert_basic(&bytes)
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        if let Some(opts) = &options {
            if let Some(ext) = &opts.file_extension {
                if ext != ".pdf" {
                    return Err(MarkitdownError::InvalidFile(format!(
                        "Expected .pdf file, got {}",
                        ext
                    )));
                }
            }
        }

        // Check if we have an LLM client for smart conversion
        if let Some(opts) = &options {
            if let Some(llm) = &opts.llm_client {
                return self.convert_with_llm(&bytes, Some(llm.as_ref())).await;
            }
        }

        self.convert_basic(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".pdf"]
    }
}

/// Configuration for PDF conversion behavior
#[derive(Debug, Clone)]
pub struct PdfConversionConfig {
    /// Minimum words required before considering LLM fallback unnecessary
    pub min_words_threshold: usize,
    /// Minimum alphanumeric ratio before text is considered garbage
    pub min_alpha_ratio: f64,
    /// Whether to always use LLM for all pages
    pub always_use_llm: bool,
}

impl Default for PdfConversionConfig {
    fn default() -> Self {
        Self {
            min_words_threshold: 10,
            min_alpha_ratio: 0.5,
            always_use_llm: false,
        }
    }
}
