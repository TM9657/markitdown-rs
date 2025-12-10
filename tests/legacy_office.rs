//! Tests for legacy Office format converters (.doc, .xls, .ppt, .rtf)
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

// Legacy Word (.doc) tests (using testOptionalHyphen.doc and testLargeOLEDoc.doc from Apache Tika)
#[tokio::test]
async fn test_doc_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("testOptionalHyphen.doc"), None).await;
    assert!(result.is_ok(), "DOC conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // DOC files should produce some output
    assert!(!markdown.is_empty(), "DOC conversion should produce output");
}

#[tokio::test]
async fn test_doc_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testOptionalHyphen.doc"))
        .expect("Failed to read testOptionalHyphen.doc");
    let options = ConversionOptions::default().with_extension(".doc");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "DOC bytes conversion failed: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_large_ole_doc_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("testLargeOLEDoc.doc"), None).await;
    assert!(
        result.is_ok(),
        "Large OLE DOC conversion failed: {:?}",
        result.err()
    );
}

// Legacy PowerPoint (.ppt) tests (using testOptionalHyphen.ppt from Apache Tika)
#[tokio::test]
async fn test_ppt_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("testOptionalHyphen.ppt"), None).await;
    assert!(result.is_ok(), "PPT conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // PPT files should produce some output
    assert!(!markdown.is_empty(), "PPT conversion should produce output");
}

#[tokio::test]
async fn test_ppt_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testOptionalHyphen.ppt"))
        .expect("Failed to read testOptionalHyphen.ppt");
    let options = ConversionOptions::default().with_extension(".ppt");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "PPT bytes conversion failed: {:?}",
        result.err()
    );
}

// RTF tests (using testOptionalHyphen.rtf from Apache Tika)
#[tokio::test]
async fn test_rtf_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("testOptionalHyphen.rtf"), None).await;
    assert!(result.is_ok(), "RTF conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(!markdown.is_empty(), "RTF conversion should produce output");
}

#[tokio::test]
async fn test_rtf_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testOptionalHyphen.rtf"))
        .expect("Failed to read testOptionalHyphen.rtf");
    let options = ConversionOptions::default().with_extension(".rtf");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "RTF bytes conversion failed: {:?}",
        result.err()
    );
}

// Word Template (.dotx) tests - should work like .docx (using testOptionalHyphen.docx from Apache Tika)
#[tokio::test]
async fn test_dotx_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testOptionalHyphen.docx"))
        .expect("Failed to read testOptionalHyphen.docx");
    let options = ConversionOptions::default().with_extension(".dotx");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(result.is_ok(), "DOTX conversion failed: {:?}", result.err());
}

// PowerPoint Template (.potx) tests - should work like .pptx (using testOptionalHyphen.pptx from Apache Tika)
#[tokio::test]
async fn test_potx_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testOptionalHyphen.pptx"))
        .expect("Failed to read testOptionalHyphen.pptx");
    let options = ConversionOptions::default().with_extension(".potx");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(result.is_ok(), "POTX conversion failed: {:?}", result.err());
}
