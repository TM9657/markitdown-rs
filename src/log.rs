//! Log file to Markdown converter.
//!
//! Parses common log file formats and converts to markdown with syntax highlighting.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use regex::Regex;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// Log file converter
pub struct LogConverter;

impl LogConverter {
    fn convert_log(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);
        let analysis = Self::analyze_log(&content);

        let mut document = Document::new();
        let mut page = Page::new(1);
        let mut markdown = String::new();

        markdown.push_str("# Log File Analysis\n\n");

        // Summary
        markdown.push_str("## Summary\n\n");
        markdown.push_str(&format!("- **Total lines:** {}\n", analysis.total_lines));
        markdown.push_str(&format!("- **Errors:** {} 🔴\n", analysis.error_count));
        markdown.push_str(&format!("- **Warnings:** {} 🟡\n", analysis.warning_count));
        markdown.push_str(&format!("- **Info:** {} 🔵\n", analysis.info_count));
        markdown.push('\n');

        // Errors section
        if !analysis.errors.is_empty() {
            markdown.push_str("## 🔴 Errors\n\n");
            for (line_num, line) in &analysis.errors {
                markdown.push_str(&format!("**Line {}:**\n```\n{}\n```\n\n", line_num, line));
            }
        }

        // Warnings section
        if !analysis.warnings.is_empty() {
            markdown.push_str("## 🟡 Warnings\n\n");
            for (line_num, line) in &analysis.warnings {
                markdown.push_str(&format!("**Line {}:**\n```\n{}\n```\n\n", line_num, line));
            }
        }

        // Full log
        markdown.push_str("## Full Log\n\n");
        markdown.push_str("```log\n");
        markdown.push_str(&content);
        if !content.ends_with('\n') {
            markdown.push('\n');
        }
        markdown.push_str("```\n");

        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);
        Ok(document)
    }

    fn analyze_log(content: &str) -> LogAnalysis {
        let mut analysis = LogAnalysis::default();

        // Common log level patterns
        let error_re =
            Regex::new(r"(?i)\b(error|fatal|critical|exception|panic|fail(ed)?)\b").unwrap();
        let warn_re = Regex::new(r"(?i)\b(warn(ing)?|caution)\b").unwrap();
        let info_re = Regex::new(r"(?i)\b(info|notice|debug|trace)\b").unwrap();

        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            analysis.total_lines += 1;

            if error_re.is_match(line) {
                analysis.error_count += 1;
                if analysis.errors.len() < 50 {
                    // Limit to first 50
                    analysis.errors.push((line_num, line.to_string()));
                }
            } else if warn_re.is_match(line) {
                analysis.warning_count += 1;
                if analysis.warnings.len() < 50 {
                    analysis.warnings.push((line_num, line.to_string()));
                }
            } else if info_re.is_match(line) {
                analysis.info_count += 1;
            }
        }

        analysis
    }
}

#[derive(Default)]
struct LogAnalysis {
    total_lines: usize,
    error_count: usize,
    warning_count: usize,
    info_count: usize,
    errors: Vec<(usize, String)>,
    warnings: Vec<(usize, String)>,
}

#[async_trait]
impl DocumentConverter for LogConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        _options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        Self::convert_log(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".log"]
    }
}
