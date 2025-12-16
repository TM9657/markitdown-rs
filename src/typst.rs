//! Typst to Markdown converter.
//!
//! Converts Typst documents to markdown.
//! Typst is a modern typesetting system.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use regex::Regex;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// Typst to Markdown converter
pub struct TypstConverter;

impl TypstConverter {
    fn convert_typst(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);
        let mut document = Document::new();
        let mut page = Page::new(1);

        let markdown = Self::typst_to_markdown(&content);
        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);

        Ok(document)
    }

    fn typst_to_markdown(content: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;
        let _in_raw = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle raw blocks ```
            if trimmed.starts_with("```") {
                if in_code_block {
                    in_code_block = false;
                    result.push_str("```\n");
                } else {
                    in_code_block = true;
                    // Extract language if present
                    let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                    result.push_str(&format!("```{}\n", lang));
                }
                continue;
            }

            if in_code_block {
                result.push_str(line);
                result.push('\n');
                continue;
            }

            // Handle `raw` inline
            if trimmed.contains('`') {
                // Keep backticks as-is for code
            }

            // Handle headings = Title, == Title, etc.
            if trimmed.starts_with('=') && !trimmed.starts_with("==") {
                let level = trimmed.chars().take_while(|&c| c == '=').count();
                let text = trimmed.trim_start_matches('=').trim();
                let hashes = "#".repeat(level);
                result.push_str(&format!("{} {}\n\n", hashes, text));
                continue;
            }

            // Handle #set, #let, #show directives (skip them)
            if trimmed.starts_with("#set")
                || trimmed.starts_with("#let")
                || trimmed.starts_with("#show")
                || trimmed.starts_with("#import")
            {
                continue;
            }

            // Handle #heading
            if trimmed.starts_with("#heading") {
                if let Some(text) = Self::extract_function_content(trimmed, "heading") {
                    result.push_str(&format!("## {}\n\n", text));
                    continue;
                }
            }

            // Handle #text, #strong, #emph
            let line = Self::convert_inline_functions(line);

            // Handle lists - Typst uses - for lists
            // (same as markdown, so no conversion needed)

            // Handle emphasis and strong
            let line = Self::convert_formatting(&line);

            result.push_str(&line);
            result.push('\n');
        }

        result
    }

    fn extract_function_content(line: &str, func: &str) -> Option<String> {
        let pattern = format!(r"#{}(?:\[[^\]]*\])?\[([^\]]+)\]", func);
        let re = Regex::new(&pattern).ok()?;
        re.captures(line).map(|c| c[1].to_string())
    }

    fn convert_inline_functions(line: &str) -> String {
        let mut result = line.to_string();

        // #strong[text] -> **text**
        let strong_re = Regex::new(r"#strong\[([^\]]+)\]").unwrap();
        result = strong_re.replace_all(&result, "**$1**").to_string();

        // #emph[text] -> *text*
        let emph_re = Regex::new(r"#emph\[([^\]]+)\]").unwrap();
        result = emph_re.replace_all(&result, "*$1*").to_string();

        // #text[text] -> text
        let text_re = Regex::new(r"#text(?:\[[^\]]*\])?\[([^\]]+)\]").unwrap();
        result = text_re.replace_all(&result, "$1").to_string();

        // #link("url")[text] -> [text](url)
        let link_re = Regex::new(r#"#link\("([^"]+)"\)\[([^\]]+)\]"#).unwrap();
        result = link_re.replace_all(&result, "[$2]($1)").to_string();

        // #link("url") -> <url>
        let link_bare_re = Regex::new(r#"#link\("([^"]+)"\)"#).unwrap();
        result = link_bare_re.replace_all(&result, "<$1>").to_string();

        result
    }

    fn convert_formatting(line: &str) -> String {
        let result = line.to_string();

        // *bold* in Typst is actually bold (not italic)
        // _italic_ in Typst
        // But Typst uses _underline_ for underline

        // For simplicity, keep * as bold marker (same as markdown)
        // The regex approach would be complex for nested cases

        result
    }
}

#[async_trait]
impl DocumentConverter for TypstConverter {
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
        Self::convert_typst(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".typ", ".typst"]
    }
}
