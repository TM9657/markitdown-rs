use crate::error::MarkitdownError;
use crate::model::{
    ContentBlock, ConversionOptions, Document, DocumentConverter, ExtractedImage, Page,
};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use quick_xml::{events::Event, reader::Reader};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;
use zip::ZipArchive;

pub struct PptxConverter;

impl PptxConverter {
    /// Detect MIME type from bytes
    fn detect_mime_type(bytes: &[u8]) -> String {
        if bytes.len() >= 8 {
            if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
                return "image/png".to_string();
            }
            if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
                return "image/jpeg".to_string();
            }
            if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
                return "image/gif".to_string();
            }
        }
        "application/octet-stream".to_string()
    }

    /// Extract images from ppt/media folder
    fn extract_images(archive: &mut ZipArchive<Cursor<&[u8]>>) -> HashMap<String, Vec<u8>> {
        let mut images = HashMap::new();

        for i in 0..archive.len() {
            if let Ok(mut file) = archive.by_index(i) {
                let name = file.name().to_string();
                if name.starts_with("ppt/media/") {
                    let mut data = Vec::new();
                    if file.read_to_end(&mut data).is_ok() {
                        // Extract just the filename
                        let filename = name.rsplit('/').next().unwrap_or(&name).to_string();
                        images.insert(filename, data);
                    }
                }
            }
        }

        images
    }

    /// Parse a single slide XML and extract text content
    fn parse_slide_content(content: &str) -> Result<(String, Vec<ContentBlock>), MarkitdownError> {
        let mut markdown = String::new();
        let mut blocks: Vec<ContentBlock> = Vec::new();
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut skip_buf = Vec::new();
        let mut count: u8 = 0;

        #[derive(Debug, Clone)]
        struct TableStat {
            #[allow(dead_code)]
            index: u8,
            rows: Vec<Vec<String>>,
        }

        loop {
            let mut found_tables: Vec<TableStat> = Vec::new();
            buf.clear();
            match reader.read_event_into(&mut buf).map_err(|e| {
                MarkitdownError::ParseError(format!("Failed to read XML event: {}", e))
            })? {
                Event::Start(element) => {
                    if element.name().as_ref() == b"p:txBody" {
                        let mut text_content = String::new();
                        let mut text_buf = Vec::new();
                        loop {
                            text_buf.clear();
                            match reader.read_event_into(&mut text_buf).map_err(|e| {
                                MarkitdownError::ParseError(format!(
                                    "Failed to read XML event: {}",
                                    e
                                ))
                            })? {
                                Event::Start(el) => {
                                    if el.name().as_ref() == b"a:t" {
                                        loop {
                                            let mut tc_buf = Vec::new();
                                            match reader.read_event_into(&mut tc_buf).map_err(
                                                |e| {
                                                    MarkitdownError::ParseError(format!(
                                                        "Failed to read XML event: {}",
                                                        e
                                                    ))
                                                },
                                            )? {
                                                Event::Text(text) => {
                                                    let decoded = text.decode().map_err(|e| {
                                                        MarkitdownError::ParseError(format!(
                                                            "Failed to decode text: {}",
                                                            e
                                                        ))
                                                    })?;
                                                    text_content.push_str(&decoded);
                                                }
                                                Event::End(el) => {
                                                    if el.name().as_ref() == b"a:t" {
                                                        break;
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                Event::End(el) => {
                                    if el.name().as_ref() == b"p:txBody" {
                                        if !text_content.is_empty() {
                                            markdown.push_str(&text_content);
                                            markdown.push_str("\n\n");
                                            blocks.push(ContentBlock::Text(text_content.clone()));
                                        }
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    if element.name().as_ref() == b"a:tbl" {
                        count += 1;
                        let mut stats = TableStat {
                            index: count,
                            rows: vec![],
                        };
                        let mut row_index = 0;
                        loop {
                            skip_buf.clear();
                            match reader.read_event_into(&mut skip_buf).map_err(|e| {
                                MarkitdownError::ParseError(format!(
                                    "Failed to read XML event: {}",
                                    e
                                ))
                            })? {
                                Event::Start(el) => match el.name().as_ref() {
                                    b"a:tr" => {
                                        stats.rows.push(vec![]);
                                        row_index = stats.rows.len() - 1;
                                    }
                                    b"a:tc" => loop {
                                        let mut tc_buf = Vec::new();
                                        match reader.read_event_into(&mut tc_buf).map_err(|e| {
                                            MarkitdownError::ParseError(format!(
                                                "Failed to read XML event: {}",
                                                e
                                            ))
                                        })? {
                                            Event::Text(text) => {
                                                let decoded = text.decode().map_err(|e| {
                                                    MarkitdownError::ParseError(format!(
                                                        "Failed to decode text: {}",
                                                        e
                                                    ))
                                                })?;
                                                stats.rows[row_index].push(decoded.to_string());
                                            }
                                            Event::End(_) => break,
                                            _ => {}
                                        }
                                    },
                                    _ => {}
                                },
                                Event::End(el) => {
                                    if el.name().as_ref() == b"a:tbl" {
                                        found_tables.push(stats);
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }

                        for t in &found_tables {
                            if !t.rows.is_empty() {
                                let headers = t.rows[0].clone();
                                let rows: Vec<Vec<String>> =
                                    t.rows.iter().skip(1).cloned().collect();
                                blocks.push(ContentBlock::Table { headers, rows });

                                // Also add to markdown
                                markdown.push('|');
                                for cell in &t.rows[0] {
                                    markdown.push_str(&format!(" {} |", cell));
                                }
                                markdown.push_str("\n|");
                                for _ in &t.rows[0] {
                                    markdown.push_str("---|");
                                }
                                markdown.push('\n');
                                for r in t.rows.iter().skip(1) {
                                    markdown.push('|');
                                    for c in r {
                                        markdown.push_str(&format!(" {} |", c));
                                    }
                                    markdown.push('\n');
                                }
                                markdown.push('\n');
                            }
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        Ok((markdown, blocks))
    }

    /// Convert bytes to Document
    fn bytes_to_document(
        &self,
        bytes: &[u8],
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let cursor = Cursor::new(bytes);
        let mut archive = ZipArchive::new(cursor)?;

        let extract_images = options.as_ref().map(|o| o.extract_images).unwrap_or(true);

        // Extract images from media folder
        let cursor2 = Cursor::new(bytes);
        let mut archive2 = ZipArchive::new(cursor2)?;
        let images = if extract_images {
            Self::extract_images(&mut archive2)
        } else {
            HashMap::new()
        };

        let mut document = Document::new();
        let mut slide_num: u32 = 1;

        // Collect slide file names and sort them
        let mut slide_files: Vec<String> = Vec::new();
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| MarkitdownError::Zip(format!("Failed to access ZIP: {}", e)))?;
            let name = file.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                slide_files.push(name);
            }
        }
        slide_files.sort();

        for slide_path in slide_files {
            let mut file = archive
                .by_name(&slide_path)
                .map_err(|e| MarkitdownError::Zip(format!("Failed to access slide: {}", e)))?;

            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| MarkitdownError::ParseError(format!("Failed to read slide: {}", e)))?;

            let (_, blocks) = Self::parse_slide_content(&content)?;

            let mut page = Page::new(slide_num);

            // Add slide header
            page.add_content(ContentBlock::Markdown(format!(
                "<!-- Slide {} -->\n",
                slide_num
            )));

            // Add parsed content blocks
            for block in blocks {
                page.add_content(block);
            }

            document.add_page(page);
            slide_num += 1;
        }

        // Add images as a separate page if any were found
        if !images.is_empty() {
            let mut image_page = Page::new(slide_num);
            image_page.add_content(ContentBlock::Heading {
                level: 2,
                text: "Embedded Images".to_string(),
            });

            let mut img_count = 0;
            for (name, data) in images {
                img_count += 1;
                let mime_type = Self::detect_mime_type(&data);
                let mut image = ExtractedImage::new(
                    format!("pptx_image_{}", img_count),
                    Bytes::from(data),
                    mime_type,
                );
                image.alt_text = Some(name);
                image_page.add_content(ContentBlock::Image(image));
            }

            document.add_page(image_page);
        }

        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for PptxConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await.map_err(|e| {
            MarkitdownError::ObjectStoreError(format!("Failed to get object: {}", e))
        })?;

        let bytes = result.bytes().await.map_err(|e| {
            MarkitdownError::ObjectStoreError(format!("Failed to read bytes: {}", e))
        })?;

        self.bytes_to_document(&bytes, options)
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        self.bytes_to_document(&bytes, options)
    }

    fn supported_extensions(&self) -> &[&str] {
        &["pptx"]
    }
}
