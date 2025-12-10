//! Legacy Office format converters (.doc, .xls, .ppt).
//!
//! These formats use OLE Compound File Binary format.
//! We extract what text we can from the streams.

use async_trait::async_trait;
use bytes::Bytes;
use cfb::CompoundFile;
use object_store::ObjectStore;
use std::io::{Cursor, Read};
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// Legacy Word document (.doc) converter
pub struct DocConverter;

impl DocConverter {
    fn convert_doc(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let cursor = Cursor::new(bytes);
        let mut cfb = CompoundFile::open(cursor)
            .map_err(|e| MarkitdownError::ParseError(format!("DOC parse error: {}", e)))?;

        let mut document = Document::new();
        let mut page = Page::new(1);
        let mut markdown = String::new();

        markdown.push_str("# Word Document (.doc)\n\n");

        // Try to read the WordDocument stream
        let text = if let Ok(mut stream) = cfb.open_stream("/WordDocument") {
            let mut data = Vec::new();
            stream.read_to_end(&mut data).ok();
            Self::extract_text_from_word_stream(&data)
        } else {
            None
        };

        // Also try the text stream if available
        let text = text.or_else(|| {
            if let Ok(mut stream) = cfb.open_stream("/1Table") {
                let mut data = Vec::new();
                stream.read_to_end(&mut data).ok();
                Self::extract_printable_text(&data)
            } else {
                None
            }
        });

        // Collect all stream paths first to avoid borrow issue
        let stream_paths: Vec<_> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_path_buf())
            .collect();

        // Fallback: try to extract any readable text from all streams
        let text = text.or_else(|| {
            let mut all_text = String::new();
            for path in &stream_paths {
                if let Ok(mut stream) = cfb.open_stream(path) {
                    let mut data = Vec::new();
                    if stream.read_to_end(&mut data).is_ok() {
                        if let Some(t) = Self::extract_printable_text(&data) {
                            all_text.push_str(&t);
                            all_text.push('\n');
                        }
                    }
                }
            }
            if all_text.is_empty() {
                None
            } else {
                Some(all_text)
            }
        });

        if let Some(text) = text {
            markdown.push_str(&text);
        } else {
            markdown.push_str("*Unable to extract text from legacy Word document.*\n\n");
            markdown.push_str("This .doc file uses a binary format. Consider converting to .docx for better extraction.\n");
        }

        // List streams for debugging - collect paths again
        let all_paths: Vec<_> = cfb.walk().map(|e| e.path().display().to_string()).collect();

        markdown.push_str("\n\n---\n\n**Document Streams:**\n");
        for path in all_paths {
            markdown.push_str(&format!("- `{}`\n", path));
        }

        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);
        Ok(document)
    }

    fn extract_text_from_word_stream(data: &[u8]) -> Option<String> {
        // Word documents store text in various encodings
        // This is a simplified extraction - real Word format is complex
        Self::extract_printable_text(data)
    }

    fn extract_printable_text(data: &[u8]) -> Option<String> {
        let mut text = String::new();
        let mut consecutive_printable = 0;
        let mut buffer = String::new();

        for &byte in data {
            if byte >= 0x20 && byte < 0x7F {
                buffer.push(byte as char);
                consecutive_printable += 1;
            } else if byte == b'\n' || byte == b'\r' || byte == b'\t' {
                buffer.push(if byte == b'\t' { ' ' } else { '\n' });
                consecutive_printable += 1;
            } else {
                // Only keep runs of printable text longer than 4 chars
                if consecutive_printable >= 4 {
                    text.push_str(&buffer);
                }
                buffer.clear();
                consecutive_printable = 0;
            }
        }

        if consecutive_printable >= 4 {
            text.push_str(&buffer);
        }

        // Clean up multiple newlines
        let text = text
            .split('\n')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        if text.len() > 20 {
            Some(text)
        } else {
            None
        }
    }
}

#[async_trait]
impl DocumentConverter for DocConverter {
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
        Self::convert_doc(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".doc"]
    }
}

/// Legacy Excel spreadsheet (.xls) converter
pub struct XlsConverter;

impl XlsConverter {
    fn convert_xls(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        // Use calamine which already supports .xls
        use calamine::{open_workbook_auto_from_rs, Reader};

        let cursor = Cursor::new(bytes.to_vec());
        let mut workbook = open_workbook_auto_from_rs(cursor)
            .map_err(|e| MarkitdownError::ParseError(format!("XLS parse error: {}", e)))?;

        let mut document = Document::new();
        let mut markdown = String::new();

        markdown.push_str("# Excel Spreadsheet (.xls)\n\n");

        let sheet_names = workbook.sheet_names().to_vec();

        for sheet_name in &sheet_names {
            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                markdown.push_str(&format!("## Sheet: {}\n\n", sheet_name));

                let rows: Vec<_> = range.rows().collect();
                if rows.is_empty() {
                    markdown.push_str("*Empty sheet*\n\n");
                    continue;
                }

                // Create table
                let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                if num_cols == 0 {
                    continue;
                }

                // Header row
                if let Some(first_row) = rows.first() {
                    let headers: Vec<String> = first_row
                        .iter()
                        .map(|c| format!("{}", c).replace('|', "\\|"))
                        .collect();
                    markdown.push_str(&format!("| {} |\n", headers.join(" | ")));
                    markdown.push_str(&format!("|{}|\n", vec!["---"; headers.len()].join("|")));
                }

                // Data rows (skip header)
                for row in rows.iter().skip(1).take(100) {
                    // Limit rows
                    let cells: Vec<String> = row
                        .iter()
                        .map(|c| format!("{}", c).replace('|', "\\|"))
                        .collect();
                    markdown.push_str(&format!("| {} |\n", cells.join(" | ")));
                }

                if rows.len() > 101 {
                    markdown.push_str(&format!("\n*... and {} more rows*\n", rows.len() - 101));
                }
                markdown.push('\n');
            }
        }

        let mut page = Page::new(1);
        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for XlsConverter {
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
        Self::convert_xls(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".xls"]
    }
}

/// Legacy PowerPoint presentation (.ppt) converter
pub struct PptConverter;

impl PptConverter {
    fn convert_ppt(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let cursor = Cursor::new(bytes);
        let mut cfb = CompoundFile::open(cursor)
            .map_err(|e| MarkitdownError::ParseError(format!("PPT parse error: {}", e)))?;

        let mut document = Document::new();
        let mut page = Page::new(1);
        let mut markdown = String::new();

        markdown.push_str("# PowerPoint Presentation (.ppt)\n\n");

        // Try to extract text from PowerPoint Document stream
        let text = if let Ok(mut stream) = cfb.open_stream("/PowerPoint Document") {
            let mut data = Vec::new();
            stream.read_to_end(&mut data).ok();
            DocConverter::extract_printable_text(&data)
        } else {
            None
        };

        if let Some(text) = text {
            markdown.push_str(&text);
        } else {
            markdown.push_str("*Unable to extract text from legacy PowerPoint document.*\n\n");
            markdown.push_str("This .ppt file uses a binary format. Consider converting to .pptx for better extraction.\n");
        }

        // List streams
        markdown.push_str("\n\n---\n\n**Document Streams:**\n");
        for entry in cfb.walk() {
            markdown.push_str(&format!("- `{}`\n", entry.path().display()));
        }

        page.add_content(ContentBlock::Markdown(markdown));
        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for PptConverter {
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
        Self::convert_ppt(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".ppt"]
    }
}

/// Word template (.dotx) converter - uses same format as .docx
pub struct DotxConverter;

#[async_trait]
impl DocumentConverter for DotxConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // DOTX uses the same format as DOCX - override extension
        let mut opts = options.unwrap_or_default();
        opts.file_extension = Some(".docx".to_string());
        let docx = crate::docx::DocxConverter;
        docx.convert(store, path, Some(opts)).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // DOTX uses the same format as DOCX - override extension
        let mut opts = options.unwrap_or_default();
        opts.file_extension = Some(".docx".to_string());
        let docx = crate::docx::DocxConverter;
        docx.convert_bytes(bytes, Some(opts)).await
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".dotx", ".dotm"]
    }
}

/// PowerPoint template (.potx) converter - uses same format as .pptx
pub struct PotxConverter;

#[async_trait]
impl DocumentConverter for PotxConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // POTX uses the same format as PPTX - override extension
        let mut opts = options.unwrap_or_default();
        opts.file_extension = Some(".pptx".to_string());
        let pptx = crate::pptx::PptxConverter;
        pptx.convert(store, path, Some(opts)).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // POTX uses the same format as PPTX - override extension
        let mut opts = options.unwrap_or_default();
        opts.file_extension = Some(".pptx".to_string());
        let pptx = crate::pptx::PptxConverter;
        pptx.convert_bytes(bytes, Some(opts)).await
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".potx", ".potm"]
    }
}

/// Excel template (.xltx) converter - uses same format as .xlsx
pub struct XltxConverter;

#[async_trait]
impl DocumentConverter for XltxConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // XLTX uses the same format as XLSX - override extension
        let mut opts = options.unwrap_or_default();
        opts.file_extension = Some(".xlsx".to_string());
        let xlsx = crate::excel::ExcelConverter;
        xlsx.convert(store, path, Some(opts)).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // XLTX uses the same format as XLSX - override extension
        let mut opts = options.unwrap_or_default();
        opts.file_extension = Some(".xlsx".to_string());
        let xlsx = crate::excel::ExcelConverter;
        xlsx.convert_bytes(bytes, Some(opts)).await
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".xltx", ".xltm"]
    }
}
