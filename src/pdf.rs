//! PDF to Markdown converter with smart LLM fallback.
//!
//! This module provides PDF conversion with intelligent handling of:
//! - Regular PDFs with extractable text
//! - Scanned PDFs (rendered as images for LLM OCR)
//! - Complex pages with diagrams/charts (sent to LLM for description)
//! - Pages with embedded images + limited text (rendered for full context)
//! - Poor quality OCR/extraction results (detected and re-processed via LLM)
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

/// Minimum ratio of valid words to total words for acceptable quality
const MIN_VALID_WORD_RATIO: f64 = 0.6;

/// Minimum average word length for text to be considered valid
const MIN_AVG_WORD_LENGTH: f64 = 2.5;

/// Maximum ratio of special characters for text to be considered valid
const MAX_SPECIAL_CHAR_RATIO: f64 = 0.3;

/// Find a good break point in text near the target position
/// Prefers breaking at paragraph boundaries, then sentences, then words
fn find_text_break_point(text: &str, target: usize) -> usize {
    if target >= text.len() {
        return text.len();
    }

    // Look for paragraph break (double newline) within 20% of target
    let search_start = (target as f64 * 0.8) as usize;
    let search_end = (target as f64 * 1.2).min(text.len() as f64) as usize;

    if let Some(pos) = text[search_start..search_end].find("\n\n") {
        return search_start + pos + 2;
    }

    // Look for sentence end
    for end_char in [". ", ".\n", "! ", "!\n", "? ", "?\n"] {
        if let Some(pos) = text[search_start..search_end].find(end_char) {
            return search_start + pos + end_char.len();
        }
    }

    // Look for word boundary
    if let Some(pos) = text[target..search_end.min(text.len())].find(' ') {
        return target + pos + 1;
    }

    target
}

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
    /// Ratio of "valid" words (reasonable length, mostly letters)
    valid_word_ratio: f64,
    /// Average word length
    avg_word_length: f64,
    /// Ratio of special/control characters
    special_char_ratio: f64,
}

impl PageMetrics {
    fn from_text_with_xobjects(text: &str, xobject_count: usize) -> Self {
        let text = text.trim().to_string();
        let char_count = text.chars().count();
        let words: Vec<&str> = text.split_whitespace().collect();
        let word_count = words.len();

        let alpha_count = text.chars().filter(|c| c.is_alphanumeric()).count();
        let alpha_ratio = if char_count > 0 {
            alpha_count as f64 / char_count as f64
        } else {
            0.0
        };

        let line_count = text.lines().count();
        let is_structured = line_count > 3 && word_count > 10;

        // Calculate valid word ratio (words that are 2-20 chars and mostly alphabetic)
        let valid_words = words
            .iter()
            .filter(|w| {
                let len = w.len();
                let alpha = w.chars().filter(|c| c.is_alphabetic()).count();
                len >= 2 && len <= 20 && (alpha as f64 / len as f64) > 0.5
            })
            .count();
        let valid_word_ratio = if word_count > 0 {
            valid_words as f64 / word_count as f64
        } else {
            0.0
        };

        // Calculate average word length
        let total_word_chars: usize = words.iter().map(|w| w.len()).sum();
        let avg_word_length = if word_count > 0 {
            total_word_chars as f64 / word_count as f64
        } else {
            0.0
        };

        // Calculate special character ratio (non-alphanumeric, non-whitespace, non-punctuation)
        let special_chars = text
            .chars()
            .filter(|c| {
                !c.is_alphanumeric() && !c.is_whitespace() && !".,:;!?'-\"()[]{}".contains(*c)
            })
            .count();
        let special_char_ratio = if char_count > 0 {
            special_chars as f64 / char_count as f64
        } else {
            0.0
        };

        Self {
            text,
            char_count,
            word_count,
            alpha_ratio,
            is_structured,
            xobject_count,
            valid_word_ratio,
            avg_word_length,
            special_char_ratio,
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

        // Poor quality text detection:
        // - Low ratio of valid words (too many garbage tokens)
        if self.valid_word_ratio < MIN_VALID_WORD_RATIO && self.word_count > 20 {
            return true;
        }

        // - Average word length too short (fragmented extraction)
        if self.avg_word_length < MIN_AVG_WORD_LENGTH && self.word_count > 10 {
            return true;
        }

        // - Too many special characters (encoding issues or garbage)
        if self.special_char_ratio > MAX_SPECIAL_CHAR_RATIO {
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

    /// Check if the extracted text appears to be high quality
    fn is_high_quality(&self) -> bool {
        self.word_count >= 10
            && self.alpha_ratio >= 0.6
            && self.valid_word_ratio >= MIN_VALID_WORD_RATIO
            && self.avg_word_length >= MIN_AVG_WORD_LENGTH
            && self.special_char_ratio <= MAX_SPECIAL_CHAR_RATIO
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

    /// Extract text aligned to actual PDF page count
    /// If text extraction gives different page count than PDF structure, redistribute text
    fn extract_text_aligned_to_pages(bytes: &[u8], actual_page_count: usize) -> Vec<String> {
        let extracted = Self::extract_text_by_page(bytes).unwrap_or_default();

        if extracted.len() == actual_page_count {
            // Perfect alignment
            return extracted;
        }

        if extracted.is_empty() || actual_page_count == 0 {
            return vec![String::new(); actual_page_count];
        }

        // Merge all text and redistribute across actual page count
        // This handles both:
        // - Fewer text pages than actual (e.g., no form feeds, all text in one block)
        // - More text pages than actual (form feeds don't align with page boundaries)
        let total_text: String = extracted.join("\n\n");

        // If total text is very small, don't try to split it
        if total_text.trim().len() < 100 {
            let mut result = vec![String::new(); actual_page_count];
            result[0] = total_text;
            return result;
        }

        let chars_per_page = (total_text.len() / actual_page_count).max(1);

        let mut result = Vec::with_capacity(actual_page_count);
        let mut remaining = total_text.as_str();

        for i in 0..actual_page_count {
            if i == actual_page_count - 1 {
                // Last page gets all remaining text
                result.push(remaining.to_string());
            } else {
                // Find a good break point near the target length
                let target_end = chars_per_page.min(remaining.len());
                let break_point = find_text_break_point(remaining, target_end);
                let (page_text, rest) = remaining.split_at(break_point);
                result.push(page_text.trim().to_string());
                remaining = rest.trim_start();
            }
        }

        result
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
        force_llm: bool,
    ) -> Result<Document, MarkitdownError> {
        // Try to parse PDF structure first to get accurate page count
        let pdf = Self::parse_pdf(bytes).ok();
        let actual_page_count = pdf.as_ref().map(|p| p.pages().len()).unwrap_or(0);

        // Get text aligned to actual page count if we have it
        let page_texts = if actual_page_count > 0 {
            Self::extract_text_aligned_to_pages(bytes, actual_page_count)
        } else {
            Self::extract_text_by_page(bytes).unwrap_or_default()
        };

        let page_count = if actual_page_count > 0 {
            actual_page_count
        } else {
            page_texts.len()
        };

        let mut document = Document::new();

        for idx in 0..page_count {
            let page_text = page_texts.get(idx).map(|s| s.as_str()).unwrap_or("");
            let xobject_count = pdf
                .as_ref()
                .map(|p| Self::count_page_xobjects(p, idx))
                .unwrap_or(0);

            let metrics = PageMetrics::from_text_with_xobjects(page_text, xobject_count);
            let page = Self::process_page(idx, &metrics, llm_client, &pdf, force_llm).await;

            if !page.content.is_empty() {
                document.add_page(page);
            }
        }

        // Handle empty document case
        if document.pages.is_empty() && page_count > 0 {
            Self::try_llm_fallback_for_all_pages(&mut document, page_count, llm_client, &pdf).await;
        }

        // Final fallback
        if document.pages.is_empty() {
            Self::add_fallback_page(&mut document);
        }

        Ok(document)
    }

    /// Process a single page with optional LLM fallback
    async fn process_page(
        idx: usize,
        metrics: &PageMetrics,
        llm_client: Option<&dyn LlmClient>,
        pdf: &Option<Pdf>,
        force_llm: bool,
    ) -> Page {
        let mut page = Page::new((idx + 1) as u32);
        let use_llm =
            (force_llm || metrics.should_use_llm()) && llm_client.is_some() && pdf.is_some();

        if use_llm {
            if let Some(content) = Self::try_llm_conversion(idx, metrics, llm_client, pdf).await {
                page.add_content(content);
                return page;
            }
        }

        // Use extracted text (either LLM not needed or LLM failed)
        if !metrics.text.is_empty() {
            // If text quality is poor and we have LLM, add a note
            if !metrics.is_high_quality() && llm_client.is_none() {
                page.add_content(ContentBlock::Text(format!(
                    "[Note: Text extraction quality may be poor]\n\n{}",
                    metrics.text
                )));
            } else {
                page.add_content(ContentBlock::Text(metrics.text.clone()));
            }
        }

        page
    }

    /// Try to convert a page using LLM
    async fn try_llm_conversion(
        idx: usize,
        metrics: &PageMetrics,
        llm_client: Option<&dyn LlmClient>,
        pdf: &Option<Pdf>,
    ) -> Option<ContentBlock> {
        let llm = llm_client?;
        let pdf_ref = pdf.as_ref()?;

        let png_data = Self::render_page_as_image(pdf_ref, idx).ok()?;

        match llm.convert_page_image(&png_data, "image/png").await {
            Ok(markdown) if !markdown.trim().is_empty() => Some(ContentBlock::Markdown(markdown)),
            _ => {
                // LLM failed or returned empty, fall back to extracted text
                if !metrics.text.is_empty() {
                    Some(ContentBlock::Text(metrics.text.clone()))
                } else {
                    None
                }
            }
        }
    }

    /// Try LLM fallback for all pages when initial extraction failed
    async fn try_llm_fallback_for_all_pages(
        document: &mut Document,
        page_count: usize,
        llm_client: Option<&dyn LlmClient>,
        pdf: &Option<Pdf>,
    ) {
        if let (Some(llm), Some(ref pdf_ref)) = (llm_client, pdf) {
            for idx in 0..page_count {
                let mut page = Page::new((idx + 1) as u32);

                if let Ok(png_data) = Self::render_page_as_image(pdf_ref, idx) {
                    if let Ok(markdown) = llm.convert_page_image(&png_data, "image/png").await {
                        if !markdown.trim().is_empty() {
                            page.add_content(ContentBlock::Markdown(markdown));
                        }
                    }
                }

                if !page.content.is_empty() {
                    document.add_page(page);
                }
            }
        }
    }

    /// Add a fallback page when nothing could be extracted
    fn add_fallback_page(document: &mut Document) {
        let mut page = Page::new(1);
        page.add_content(ContentBlock::Text(
            "[PDF content could not be extracted. The document may be scanned, protected, or use unsupported encoding.]"
                .to_string(),
        ));
        document.add_page(page);
    }

    /// Basic conversion without LLM (original behavior)
    fn convert_basic(&self, bytes: &[u8]) -> Result<Document, MarkitdownError> {
        // Try to get actual page count from PDF structure
        let pdf = Self::parse_pdf(bytes).ok();
        let actual_page_count = pdf.as_ref().map(|p| p.pages().len()).unwrap_or(0);

        // Get text aligned to actual pages if possible
        let page_texts = if actual_page_count > 0 {
            Self::extract_text_aligned_to_pages(bytes, actual_page_count)
        } else {
            let text_content = pdf_extract::extract_text_from_mem(bytes).map_err(|e| {
                MarkitdownError::ParseError(format!("Failed to extract text from PDF: {}", e))
            })?;
            text_content.split('\x0c').map(|s| s.to_string()).collect()
        };

        let mut document = Document::new();

        for (idx, page_text) in page_texts.iter().enumerate() {
            let trimmed = page_text.trim();
            if !trimmed.is_empty() {
                let mut page = Page::new((idx + 1) as u32);
                page.add_content(ContentBlock::Text(trimmed.to_string()));
                document.add_page(page);
            }
        }

        // If we have actual page count but no content, create empty pages
        if document.pages.is_empty() && actual_page_count > 0 {
            for idx in 0..actual_page_count {
                let mut page = Page::new((idx + 1) as u32);
                page.add_content(ContentBlock::Text(
                    "[Page content could not be extracted]".to_string(),
                ));
                document.add_page(page);
            }
        } else if document.pages.is_empty() {
            // Fallback: single page with all text
            let text_content = pdf_extract::extract_text_from_mem(bytes)
                .map(|t| t.trim().to_string())
                .unwrap_or_default();
            let mut page = Page::new(1);
            if text_content.is_empty() {
                page.add_content(ContentBlock::Text(
                    "[PDF content could not be extracted]".to_string(),
                ));
            } else {
                page.add_content(ContentBlock::Text(text_content));
            }
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
                let force_llm = opts.force_llm_ocr;
                return self
                    .convert_with_llm(&bytes, Some(llm.as_ref()), force_llm)
                    .await;
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
                let force_llm = opts.force_llm_ocr;
                return self
                    .convert_with_llm(&bytes, Some(llm.as_ref()), force_llm)
                    .await;
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
