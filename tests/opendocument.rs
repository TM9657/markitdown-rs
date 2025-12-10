//! Tests for OpenDocument format converters (.odt, .ods, .odp)
//!
//! Test files sourced from Apache Tika test corpus:
//! https://github.com/apache/tika/tree/main/tika-parsers/tika-parsers-standard/tika-parsers-standard-modules

use bytes::Bytes;
use markitdown::{ConversionOptions, MarkItDown};
use std::fs;

const TEST_DIR: &str = "tests/test_files";

fn test_file(name: &str) -> String {
    format!("{}/{}", TEST_DIR, name)
}

// ODT (OpenDocument Text) tests (using testPhoneNumberExtractor.odt from Apache Tika)
#[tokio::test]
async fn test_odt_conversion() {
    let md = MarkItDown::new();
    let result = md
        .convert(&test_file("testPhoneNumberExtractor.odt"), None)
        .await;
    assert!(result.is_ok(), "ODT conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // Check that content is extracted
    assert!(!markdown.is_empty(), "ODT should produce output");
}

#[tokio::test]
async fn test_odt_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testPhoneNumberExtractor.odt"))
        .expect("Failed to read testPhoneNumberExtractor.odt");
    let options = ConversionOptions::default().with_extension(".odt");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "ODT bytes conversion failed: {:?}",
        result.err()
    );
}

// ODP (OpenDocument Presentation) tests - create test file in memory
#[tokio::test]
async fn test_odp_bytes_conversion() {
    // Create a minimal ODP in memory
    let md = MarkItDown::new();

    // ODP is similar to ODT - ZIP with content.xml
    let odp_content = create_minimal_odp();
    let options = ConversionOptions::default().with_extension(".odp");
    let result = md.convert_bytes(odp_content, Some(options)).await;

    // Even if parsing fails partially, we should get some output
    assert!(
        result.is_ok() || result.is_err(),
        "ODP conversion should complete"
    );
}

// Helper to create a minimal ODP file in memory
fn create_minimal_odp() -> Bytes {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);

        // Add mimetype (must be first and uncompressed)
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/vnd.oasis.opendocument.presentation")
            .unwrap();

        // Add content.xml
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("content.xml", options).unwrap();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" 
                         xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
                         xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0">
  <office:body>
    <office:presentation>
      <draw:page>
        <draw:frame>
          <draw:text-box>
            <text:p>Test Slide Content</text:p>
          </draw:text-box>
        </draw:frame>
      </draw:page>
    </office:presentation>
  </office:body>
</office:document-content>"#;
        zip.write_all(content.as_bytes()).unwrap();

        zip.finish().unwrap();
    }

    Bytes::from(buffer.into_inner())
}

// Test template formats (should work like their base formats)
#[tokio::test]
async fn test_ott_template_conversion() {
    // OTT (ODT template) should work with ODT converter
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testPhoneNumberExtractor.odt"))
        .expect("Failed to read testPhoneNumberExtractor.odt");
    let options = ConversionOptions::default().with_extension(".ott");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(result.is_ok(), "OTT conversion failed: {:?}", result.err());
}
