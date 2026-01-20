//! BibTeX (.bib) to Markdown converter.
//!
//! Parses BibTeX bibliography files and converts to markdown.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// BibTeX file converter
pub struct BibtexConverter;

/// Represents a single BibTeX entry
#[derive(Debug, Default)]
struct BibEntry {
    entry_type: String,
    cite_key: String,
    fields: HashMap<String, String>,
}

impl BibtexConverter {
    fn convert_bib(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);
        let entries = Self::parse_bibtex(&content)?;

        let mut document = Document::new();
        let mut page = Page::new(1);
        let mut markdown = String::new();

        markdown.push_str("# Bibliography\n\n");
        markdown.push_str(&format!("*{} entries*\n\n", entries.len()));

        for entry in &entries {
            markdown.push_str(&Self::format_entry(entry));
        }

        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);
        Ok(document)
    }

    fn parse_bibtex(content: &str) -> Result<Vec<BibEntry>, MarkitdownError> {
        let mut entries = Vec::new();

        // Match field = {value} or field = "value" or field = value
        let field_re =
            Regex::new(r#"(\w+)\s*=\s*(?:\{([^{}]*(?:\{[^{}]*\}[^{}]*)*)\}|"([^"]*)"|(\d+))"#)
                .map_err(|e| MarkitdownError::ParseError(format!("Regex error: {}", e)))?;

        // Simple state-machine parsing for BibTeX entries
        // Find each @type{key, ... }
        let mut chars = content.char_indices().peekable();

        while let Some((_, c)) = chars.next() {
            if c == '@' {
                // Found start of entry
                // Get entry type
                let mut entry_type = String::new();
                while let Some((_, c)) = chars.peek() {
                    if c.is_alphanumeric() || *c == '_' {
                        entry_type.push(*c);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Skip whitespace
                while let Some((_, c)) = chars.peek() {
                    if c.is_whitespace() {
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Expect {
                if let Some((_, '{')) = chars.peek() {
                    chars.next();
                } else {
                    continue;
                }

                // Skip whitespace
                while let Some((_, c)) = chars.peek() {
                    if c.is_whitespace() {
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Get cite key (until comma or whitespace)
                let mut cite_key = String::new();
                while let Some((_, c)) = chars.peek() {
                    if *c == ',' || c.is_whitespace() {
                        break;
                    }
                    cite_key.push(*c);
                    chars.next();
                }

                // Find matching closing brace
                let mut brace_count = 1;
                let fields_start = chars.peek().map(|(i, _)| *i).unwrap_or(content.len());
                let mut fields_end = fields_start;

                for (idx, c) in chars.by_ref() {
                    if c == '{' {
                        brace_count += 1;
                    } else if c == '}' {
                        brace_count -= 1;
                        if brace_count == 0 {
                            fields_end = idx;
                            break;
                        }
                    }
                }

                if !entry_type.is_empty() && !cite_key.is_empty() {
                    let mut entry = BibEntry {
                        entry_type: entry_type.to_lowercase(),
                        cite_key,
                        fields: HashMap::new(),
                    };

                    // Parse fields from the content between braces
                    let fields_str = &content[fields_start..fields_end];
                    for field_cap in field_re.captures_iter(fields_str) {
                        let key = field_cap[1].to_lowercase();
                        let value = field_cap
                            .get(2)
                            .or(field_cap.get(3))
                            .or(field_cap.get(4))
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_default();
                        entry.fields.insert(key, value);
                    }

                    entries.push(entry);
                }
            }
        }

        Ok(entries)
    }

    fn format_entry(entry: &BibEntry) -> String {
        let mut md = String::new();

        // Entry type icon
        let icon = match entry.entry_type.as_str() {
            "article" => "📄",
            "book" | "inbook" => "📚",
            "conference" | "inproceedings" => "🎤",
            "phdthesis" | "mastersthesis" => "🎓",
            "techreport" => "📋",
            "misc" | "online" => "🔗",
            _ => "📝",
        };

        md.push_str(&format!(
            "## {} {} `{}`\n\n",
            icon,
            Self::capitalize(&entry.entry_type),
            entry.cite_key
        ));

        // Title
        if let Some(title) = entry.fields.get("title") {
            md.push_str(&format!("**{}**\n\n", Self::clean_latex(title)));
        }

        // Authors
        if let Some(author) = entry.fields.get("author") {
            md.push_str(&format!("*{}*\n\n", Self::clean_latex(author)));
        }

        // Publication info
        let mut pub_info = Vec::new();

        if let Some(journal) = entry.fields.get("journal") {
            pub_info.push(format!("**Journal:** {}", Self::clean_latex(journal)));
        }
        if let Some(booktitle) = entry.fields.get("booktitle") {
            pub_info.push(format!("**In:** {}", Self::clean_latex(booktitle)));
        }
        if let Some(publisher) = entry.fields.get("publisher") {
            pub_info.push(format!("**Publisher:** {}", Self::clean_latex(publisher)));
        }
        if let Some(year) = entry.fields.get("year") {
            pub_info.push(format!("**Year:** {}", year));
        }
        if let Some(volume) = entry.fields.get("volume") {
            pub_info.push(format!("**Volume:** {}", volume));
        }
        if let Some(pages) = entry.fields.get("pages") {
            pub_info.push(format!("**Pages:** {}", pages));
        }
        if let Some(doi) = entry.fields.get("doi") {
            pub_info.push(format!("**DOI:** [{}](https://doi.org/{})", doi, doi));
        }
        if let Some(url) = entry.fields.get("url") {
            pub_info.push(format!("**URL:** <{}>", url));
        }

        if !pub_info.is_empty() {
            for info in pub_info {
                md.push_str(&format!("- {}\n", info));
            }
            md.push('\n');
        }

        // Abstract
        if let Some(abs) = entry.fields.get("abstract") {
            md.push_str(&format!("> {}\n\n", Self::clean_latex(abs)));
        }

        md.push_str("---\n\n");
        md
    }

    fn capitalize(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    fn clean_latex(s: &str) -> String {
        // Remove common LaTeX commands and braces
        s.replace(['{', '}'], "")
            .replace("\\&", "&")
            .replace("\\%", "%")
            .replace("\\textit", "")
            .replace("\\textbf", "")
            .replace("\\emph", "")
            .replace("\\url", "")
            .replace("  ", " ")
            .trim()
            .to_string()
    }
}

#[async_trait]
impl DocumentConverter for BibtexConverter {
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
        Self::convert_bib(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".bib", ".bibtex"]
    }
}
