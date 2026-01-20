//! Tests for legacy Office format converters (.doc, .xls, .ppt, .rtf)
//!
//! Test files sourced from kreuzberg test documents

use bytes::Bytes;
use markitdown::{ConversionOptions, MarkItDown};
use std::fs;

const TEST_DIR: &str = "tests/test_documents";

fn test_file(name: &str) -> String {
    format!("{}/{}", TEST_DIR, name)
}

// ============================================================================
// Legacy Word (.doc) tests
// ============================================================================

#[tokio::test]
async fn test_doc_conversion() {
    let md = MarkItDown::new();
    let result = md
        .convert(&test_file("legacy_office/unit_test_lists.doc"), None)
        .await;
    assert!(result.is_ok(), "DOC conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // DOC files should produce some output
    assert!(!markdown.is_empty(), "DOC conversion should produce output");
}

#[tokio::test]
async fn test_doc_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("legacy_office/unit_test_lists.doc"))
        .expect("Failed to read unit_test_lists.doc");
    let options = ConversionOptions::default().with_extension(".doc");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "DOC bytes conversion failed: {:?}",
        result.err()
    );
}

// ============================================================================
// Legacy PowerPoint (.ppt) tests
// ============================================================================

#[tokio::test]
async fn test_ppt_conversion() {
    let md = MarkItDown::new();
    let result = md
        .convert(&test_file("legacy_office/simple.ppt"), None)
        .await;
    assert!(result.is_ok(), "PPT conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // PPT files should produce some output
    assert!(!markdown.is_empty(), "PPT conversion should produce output");
}

#[tokio::test]
async fn test_ppt_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("legacy_office/simple.ppt")).expect("Failed to read simple.ppt");
    let options = ConversionOptions::default().with_extension(".ppt");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "PPT bytes conversion failed: {:?}",
        result.err()
    );
}

// ============================================================================
// RTF tests
// ============================================================================

#[tokio::test]
async fn test_rtf_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("rtf/lorem_ipsum.rtf"), None).await;
    assert!(result.is_ok(), "RTF conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(!markdown.is_empty(), "RTF conversion should produce output");
}

#[tokio::test]
async fn test_rtf_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("rtf/lorem_ipsum.rtf")).expect("Failed to read lorem_ipsum.rtf");
    let options = ConversionOptions::default().with_extension(".rtf");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "RTF bytes conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_rtf_formatting() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("rtf/formatting.rtf"), None).await;
    assert!(result.is_ok(), "RTF conversion failed: {:?}", result.err());
}

#[tokio::test]
async fn test_rtf_tables() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("rtf/table_simple.rtf"), None).await;
    assert!(
        result.is_ok(),
        "RTF table conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_rtf_unicode() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("rtf/unicode.rtf"), None).await;
    assert!(
        result.is_ok(),
        "RTF unicode conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_rtf_headings() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("rtf/heading.rtf"), None).await;
    assert!(
        result.is_ok(),
        "RTF heading conversion failed: {:?}",
        result.err()
    );
}

// ============================================================================
// Word Template (.dotx) tests - should work like .docx
// ============================================================================

#[tokio::test]
async fn test_dotx_conversion() {
    let md = MarkItDown::new();
    // Use a DOCX file with DOTX extension to test template handling
    let bytes = fs::read(test_file("office/document.docx")).expect("Failed to read document.docx");
    let options = ConversionOptions::default().with_extension(".dotx");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(result.is_ok(), "DOTX conversion failed: {:?}", result.err());
}

// ============================================================================
// PowerPoint Template (.potx) tests - should work like .pptx
// ============================================================================

#[tokio::test]
async fn test_potx_conversion() {
    let md = MarkItDown::new();
    // Use a PPTX file with POTX extension to test template handling
    let bytes =
        fs::read(test_file("presentations/simple.pptx")).expect("Failed to read simple.pptx");
    let options = ConversionOptions::default().with_extension(".potx");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(result.is_ok(), "POTX conversion failed: {:?}", result.err());
}

// ============================================================================
// Image extraction tests for legacy formats
// ============================================================================

/// Test that image extraction is enabled by default for PPT files
#[tokio::test]
async fn test_ppt_image_extraction_enabled() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("legacy_office/simple.ppt")).expect("Failed to read simple.ppt");

    // Test with default options (images should be extracted)
    let options = ConversionOptions::default()
        .with_extension(".ppt")
        .with_images(true);
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "PPT conversion with image extraction failed: {:?}",
        result.err()
    );

    // The simple.ppt file has no images, so this verifies the code path works without errors
    let doc = result.unwrap();
    let images = doc.images();
    println!("PPT images extracted: {}", images.len());
    // simple.ppt has no images (Pictures stream is empty)
    assert_eq!(images.len(), 0, "simple.ppt should have no images");
}

/// Test that image extraction is enabled by default for DOC files
#[tokio::test]
async fn test_doc_image_extraction_enabled() {
    let md = MarkItDown::new();
    let bytes =
        fs::read(test_file("legacy_office/unit_test_lists.doc")).expect("Failed to read DOC");

    // Test with default options (images should be extracted)
    let options = ConversionOptions::default()
        .with_extension(".doc")
        .with_images(true);
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "DOC conversion with image extraction failed: {:?}",
        result.err()
    );

    // The unit_test_lists.doc file has no images
    let doc = result.unwrap();
    let images = doc.images();
    println!("DOC images extracted: {}", images.len());
    // unit_test_lists.doc has no images
    assert_eq!(images.len(), 0, "unit_test_lists.doc should have no images");
}

/// Test that image extraction can be disabled
#[tokio::test]
async fn test_legacy_image_extraction_disabled() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("legacy_office/simple.ppt")).expect("Failed to read simple.ppt");

    // Disable image extraction
    let options = ConversionOptions::default()
        .with_extension(".ppt")
        .with_images(false);
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "PPT conversion without images failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let images = doc.images();
    assert_eq!(
        images.len(),
        0,
        "No images should be extracted when disabled"
    );
}
