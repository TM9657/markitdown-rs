//! reStructuredText to Markdown converter.
//!
//! Converts RST files to markdown using basic regex-based parsing.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use regex::Regex;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// reStructuredText to Markdown converter
pub struct RstConverter;

impl RstConverter {
    fn convert_rst(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);
        let mut document = Document::new();
        let mut page = Page::new(1);

        let markdown = Self::rst_to_markdown(&content);
        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);

        Ok(document)
    }

    fn rst_to_markdown(content: &str) -> String {
        let mut result = String::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut i = 0;
        let mut in_code_block = false;

        while i < lines.len() {
            let line = lines[i];

            // Handle code blocks (:: at end of line starts it)
            if line.trim().ends_with("::") {
                let text = line.trim().trim_end_matches("::");
                if !text.is_empty() {
                    result.push_str(text);
                    result.push('\n');
                }
                result.push_str("```\n");
                in_code_block = true;
                i += 1;

                // Skip blank line after ::
                if i < lines.len() && lines[i].trim().is_empty() {
                    i += 1;
                }
                continue;
            }

            // End code block when indentation decreases
            if in_code_block {
                if line.trim().is_empty() || line.starts_with("    ") || line.starts_with('\t') {
                    // Remove leading indentation
                    let dedented = line
                        .strip_prefix("    ")
                        .or_else(|| line.strip_prefix('\t'))
                        .unwrap_or(line);
                    result.push_str(dedented);
                    result.push('\n');
                } else {
                    in_code_block = false;
                    result.push_str("```\n");
                    // Process this line normally
                    continue;
                }
                i += 1;
                continue;
            }

            // Handle titles with underlines
            if i + 1 < lines.len() {
                let next_line = lines[i + 1];
                let underline_chars = ['=', '-', '~', '^', '"', '+', '*'];

                if !next_line.is_empty()
                    && next_line.len() >= line.len()
                    && underline_chars.contains(&next_line.chars().next().unwrap_or(' '))
                    && next_line.chars().all(|c| c == next_line.chars().next().unwrap())
                {
                    let level = match next_line.chars().next().unwrap() {
                        '=' => 1,
                        '-' => 2,
                        '~' => 3,
                        '^' => 4,
                        _ => 5,
                    };
                    let hashes = "#".repeat(level);
                    result.push_str(&format!("{} {}\n", hashes, line.trim()));
                    i += 2;
                    continue;
                }
            }

            // Handle inline formatting
            let line = Self::convert_formatting(line);

            // Handle links
            let line = Self::convert_links(&line);

            // Handle directives (skip most, but handle some)
            if line.trim().starts_with("..") {
                // Skip directives and their content
                i += 1;
                continue;
            }

            result.push_str(&line);
            result.push('\n');
            i += 1;
        }

        // Close any open code block
        if in_code_block {
            result.push_str("```\n");
        }

        result
    }

    fn convert_formatting(line: &str) -> String {
        let mut result = line.to_string();

        // Bold: **text** stays the same
        // Italic: *text* stays the same
        // Code: ``text`` -> `text`
        let code_re = Regex::new(r"``([^`]+)``").unwrap();
        result = code_re.replace_all(&result, "`$1`").to_string();

        result
    }

    fn convert_links(line: &str) -> String {
        // `text <url>`_ -> [text](url)
        let link_re = Regex::new(r"`([^<]+)\s+<([^>]+)>`_").unwrap();
        link_re.replace_all(line, "[$1]($2)").to_string()
    }
}

#[async_trait]
impl DocumentConverter for RstConverter {
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
        Self::convert_rst(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".rst", ".rest"]
    }
}
