//! Markdown pass-through converter.
//!
//! Handles markdown files, optionally normalizing them.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// Markdown file converter (pass-through)
pub struct MarkdownConverter;

impl MarkdownConverter {
    fn convert_markdown(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);

        let mut document = Document::new();
        let mut page = Page::new(1);

        // Clean up the markdown (normalize line endings, trim)
        let cleaned = content
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .trim()
            .to_string();

        page.add_content(ContentBlock::Markdown(cleaned));
        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for MarkdownConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let valid_extensions = [".md", ".markdown", ".mdown", ".mkd"];

        if let Some(opts) = &options {
            if let Some(ext) = &opts.file_extension {
                if !valid_extensions.contains(&ext.as_str()) {
                    return Err(MarkitdownError::InvalidFile(format!(
                        "Expected Markdown file, got {}",
                        ext
                    )));
                }
            }
        }

        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let valid_extensions = [".md", ".markdown", ".mdown", ".mkd"];

        if let Some(opts) = &options {
            if let Some(ext) = &opts.file_extension {
                if !valid_extensions.contains(&ext.as_str()) {
                    return Err(MarkitdownError::InvalidFile(format!(
                        "Expected Markdown file, got {}",
                        ext
                    )));
                }
            }
        }

        Self::convert_markdown(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".md", ".markdown", ".mdown", ".mkd"]
    }
}
