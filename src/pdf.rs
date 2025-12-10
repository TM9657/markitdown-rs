use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use pdf_extract;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

pub struct PdfConverter;

impl PdfConverter {
    fn convert_pdf_bytes(&self, bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let text_content = pdf_extract::extract_text_from_mem(bytes).map_err(|e| {
            MarkitdownError::ParseError(format!("Failed to extract text from PDF: {}", e))
        })?;

        let mut document = Document::new();

        // Split text by page markers or just create a single page
        let pages: Vec<&str> = text_content.split("\x0c").collect(); // Form feed often separates pages

        for (idx, page_text) in pages.iter().enumerate() {
            let trimmed = page_text.trim();
            if !trimmed.is_empty() {
                let mut page = Page::new((idx + 1) as u32);
                page.add_content(ContentBlock::Text(trimmed.to_string()));
                document.add_page(page);
            }
        }

        // If no pages were created, create at least one with the full content
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
        self.convert_pdf_bytes(&bytes)
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

        self.convert_pdf_bytes(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".pdf"]
    }
}
