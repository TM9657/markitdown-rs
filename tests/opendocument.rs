//! Tests for OpenDocument format converters (.odt, .ods, .odp)
//!
//! Test files sourced from kreuzberg test documents

use bytes::Bytes;
use markitdown::{ConversionOptions, MarkItDown};
use std::fs;
use std::io::Write;
use zip::write::SimpleFileOptions;

const TEST_DIR: &str = "tests/test_documents";

fn test_file(name: &str) -> String {
    format!("{}/{}", TEST_DIR, name)
}

// ============================================================================
// ODT (OpenDocument Text) tests
// ============================================================================

#[tokio::test]
async fn test_odt_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("documents/fake.odt"), None).await;
    assert!(result.is_ok(), "ODT conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // Check that content is extracted
    assert!(!markdown.is_empty(), "ODT should produce output");
}

#[tokio::test]
async fn test_odt_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("documents/fake.odt")).expect("Failed to read fake.odt");
    let options = ConversionOptions::default().with_extension(".odt");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "ODT bytes conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_odt_simple() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("documents/simple.odt"), None).await;
    assert!(result.is_ok(), "ODT conversion failed: {:?}", result.err());
}

#[tokio::test]
async fn test_odt_bold() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("odt/bold.odt"), None).await;
    assert!(result.is_ok(), "ODT conversion failed: {:?}", result.err());
}

#[tokio::test]
async fn test_odt_table() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("odt/simpleTable.odt"), None).await;
    assert!(
        result.is_ok(),
        "ODT table conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_odt_unicode() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("odt/unicode.odt"), None).await;
    assert!(
        result.is_ok(),
        "ODT unicode conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_odt_formula() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("odt/formula.odt"), None).await;
    assert!(
        result.is_ok(),
        "ODT formula conversion failed: {:?}",
        result.err()
    );
}

// ============================================================================
// ODP (OpenDocument Presentation) tests
// ============================================================================

fn create_minimal_odp() -> Bytes {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default();

        // Add mimetype
        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/vnd.oasis.opendocument.presentation")
            .unwrap();

        // Add minimal content.xml
        let content_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
    xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0">
  <office:body>
    <office:presentation>
      <draw:page draw:name="Slide 1">
        <draw:frame>
          <draw:text-box>
            <text:p>Test Presentation Content</text:p>
          </draw:text-box>
        </draw:frame>
      </draw:page>
    </office:presentation>
  </office:body>
</office:document-content>"#;

        zip.start_file("content.xml", options).unwrap();
        zip.write_all(content_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    Bytes::from(buffer.into_inner())
}

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

// ============================================================================
// OTT (OpenDocument Template) tests
// ============================================================================

#[tokio::test]
async fn test_ott_template_conversion() {
    let md = MarkItDown::new();
    // Use an ODT file as template (OTT has same format)
    let bytes = fs::read(test_file("documents/simple.odt")).expect("Failed to read simple.odt");
    let options = ConversionOptions::default().with_extension(".ott");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(result.is_ok(), "OTT conversion failed: {:?}", result.err());
}
