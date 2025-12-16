//! FictionBook (FB2) to Markdown converter.
//!
//! Converts FB2 ebook files to markdown.
//! FB2 is an XML-based ebook format popular in Russia.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// FictionBook (FB2) to Markdown converter
pub struct FictionBookConverter;

impl FictionBookConverter {
    fn convert_fb2(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);
        let mut document = Document::new();

        let (metadata, body_pages) = Self::fb2_to_markdown(&content)?;

        // Add metadata page
        if !metadata.is_empty() {
            let mut meta_page = Page::new(0);
            meta_page.add_content(ContentBlock::Markdown(metadata));
            document.add_page(meta_page);
        }

        // Add content pages
        for (i, content) in body_pages.into_iter().enumerate() {
            let mut page = Page::new((i + 1) as u32);
            page.add_content(ContentBlock::Markdown(content));
            document.add_page(page);
        }

        Ok(document)
    }

    fn fb2_to_markdown(content: &str) -> Result<(String, Vec<String>), MarkitdownError> {
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut metadata = String::new();
        let mut pages: Vec<String> = Vec::new();
        let mut current_page = String::new();

        let mut buf = Vec::new();
        let mut text_buf = String::new();

        // State tracking
        let mut in_description = false;
        let mut in_title_info = false;
        let mut in_body = false;
        let mut in_section = false;
        let mut in_title = false;
        let mut in_paragraph = false;
        let mut in_emphasis = false;
        let mut in_strong = false;
        let mut in_epigraph = false;
        let mut in_poem = false;
        let mut in_stanza = false;
        let mut in_v = false; // verse line

        let mut book_title = String::new();
        let mut authors: Vec<String> = Vec::new();
        let mut current_author = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"description" => in_description = true,
                    b"title-info" => in_title_info = true,
                    b"body" => in_body = true,
                    b"section" => {
                        in_section = true;
                        if !current_page.is_empty() {
                            pages.push(std::mem::take(&mut current_page));
                        }
                    }
                    b"title" => in_title = true,
                    b"p" => in_paragraph = true,
                    b"emphasis" => in_emphasis = true,
                    b"strong" => in_strong = true,
                    b"epigraph" => in_epigraph = true,
                    b"poem" => in_poem = true,
                    b"stanza" => in_stanza = true,
                    b"v" => in_v = true,
                    b"author" if in_title_info => {
                        current_author.clear();
                    }
                    b"book-title" if in_title_info => {}
                    _ => {}
                },
                Ok(Event::End(ref e)) => match e.name().as_ref() {
                    b"description" => {
                        in_description = false;
                        // Build metadata
                        if !book_title.is_empty() {
                            metadata.push_str(&format!("# {}\n\n", book_title));
                        }
                        for author in &authors {
                            metadata.push_str(&format!("**Author:** {}\n\n", author));
                        }
                        if !metadata.is_empty() {
                            metadata.push_str("---\n\n");
                        }
                    }
                    b"title-info" => in_title_info = false,
                    b"body" => {
                        in_body = false;
                        if !current_page.is_empty() {
                            pages.push(std::mem::take(&mut current_page));
                        }
                    }
                    b"section" => in_section = false,
                    b"title" => {
                        if !text_buf.is_empty() {
                            let level = if in_section { "##" } else { "#" };
                            current_page.push_str(&format!("{} {}\n\n", level, text_buf.trim()));
                            text_buf.clear();
                        }
                        in_title = false;
                    }
                    b"p" => {
                        if !text_buf.is_empty() {
                            if in_epigraph {
                                current_page.push_str(&format!("> {}\n", text_buf.trim()));
                            } else if in_poem || in_stanza {
                                current_page.push_str(&format!("*{}*\n", text_buf.trim()));
                            } else {
                                current_page.push_str(&format!("{}\n\n", text_buf.trim()));
                            }
                            text_buf.clear();
                        }
                        in_paragraph = false;
                    }
                    b"emphasis" => in_emphasis = false,
                    b"strong" => in_strong = false,
                    b"epigraph" => {
                        in_epigraph = false;
                        current_page.push('\n');
                    }
                    b"poem" => {
                        in_poem = false;
                        current_page.push('\n');
                    }
                    b"stanza" => {
                        in_stanza = false;
                        current_page.push('\n');
                    }
                    b"v" => {
                        if !text_buf.is_empty() {
                            current_page.push_str(&format!("*{}*  \n", text_buf.trim()));
                            text_buf.clear();
                        }
                        in_v = false;
                    }
                    b"author" if in_title_info => {
                        if !current_author.is_empty() {
                            authors.push(std::mem::take(&mut current_author));
                        }
                    }
                    b"book-title" if in_title_info => {
                        book_title = std::mem::take(&mut text_buf);
                    }
                    b"first-name" | b"middle-name" | b"last-name" if in_title_info => {
                        if !text_buf.is_empty() {
                            if !current_author.is_empty() {
                                current_author.push(' ');
                            }
                            current_author.push_str(text_buf.trim());
                            text_buf.clear();
                        }
                    }
                    _ => {}
                },
                Ok(Event::Text(e)) => {
                    let text = e.decode().map(|s| s.to_string()).unwrap_or_default();

                    if in_description && in_title_info {
                        text_buf.push_str(&text);
                    } else if in_body {
                        // Apply formatting
                        let formatted = if in_strong {
                            format!("**{}**", text)
                        } else if in_emphasis {
                            format!("*{}*", text)
                        } else {
                            text
                        };

                        if in_title || in_paragraph || in_v {
                            text_buf.push_str(&formatted);
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(MarkitdownError::ParseError(format!(
                        "FB2 parse error: {}",
                        e
                    )));
                }
                _ => {}
            }
            buf.clear();
        }

        Ok((metadata, pages))
    }
}

#[async_trait]
impl DocumentConverter for FictionBookConverter {
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
        Self::convert_fb2(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".fb2"]
    }
}
