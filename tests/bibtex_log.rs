//! Tests for BibTeX and log file converters
//!
//! Test files sourced from kreuzberg test documents

use bytes::Bytes;
use markitdown::{ConversionOptions, MarkItDown};

const TEST_DIR: &str = "tests/test_documents";

fn test_file(name: &str) -> String {
    format!("{}/{}", TEST_DIR, name)
}

// BibTeX tests
#[tokio::test]
async fn test_bibtex_conversion() {
    let md = MarkItDown::new();
    let result = md
        .convert(&test_file("bibtex/comprehensive.bib"), None)
        .await;
    assert!(
        result.is_ok(),
        "BibTeX conversion failed: {:?}",
        result.err()
    );
    let doc = result.unwrap();
    let markdown = doc.to_markdown();

    // Check that entries are parsed
    assert!(!markdown.is_empty(), "BibTeX should produce output");
}

#[tokio::test]
async fn test_bibtex_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = std::fs::read(test_file("bibtex/comprehensive.bib"))
        .expect("Failed to read comprehensive.bib");
    let options = ConversionOptions::default().with_extension(".bib");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "BibTeX bytes conversion failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(!markdown.is_empty(), "Should produce some output");
}

#[tokio::test]
async fn test_bibtex_with_special_characters() {
    let md = MarkItDown::new();
    let bibtex_content = r#"
@article{test2024,
  author = {Müller, Hans and O'Brien, John},
  title = {Special Characters: äöü & "quotes"},
  year = {2024}
}
"#;
    let options = ConversionOptions::default().with_extension(".bib");
    let result = md
        .convert_bytes(Bytes::from(bibtex_content), Some(options))
        .await;
    assert!(
        result.is_ok(),
        "BibTeX with special chars failed: {:?}",
        result.err()
    );
}

// Log file tests (using in-memory content)
#[tokio::test]
async fn test_log_bytes_conversion() {
    let md = MarkItDown::new();
    let log_content = "2024-01-15 10:30:45 INFO Application started successfully
2024-01-15 10:30:46 DEBUG Initializing components
2024-01-15 10:30:47 WARN Low memory detected
2024-01-15 10:30:48 ERROR Connection failed to database
2024-01-15 10:30:49 INFO Retrying connection...";
    let options = ConversionOptions::default().with_extension(".log");
    let result = md
        .convert_bytes(Bytes::from(log_content), Some(options))
        .await;
    assert!(
        result.is_ok(),
        "Log bytes conversion failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(
        markdown.contains("INFO") || markdown.contains("ERROR"),
        "Should contain log levels"
    );
}

#[tokio::test]
async fn test_log_wrapped_in_code_block() {
    let md = MarkItDown::new();
    let log_content = "2024-01-15 INFO Test message\n2024-01-15 ERROR Another message";
    let options = ConversionOptions::default().with_extension(".log");
    let result = md
        .convert_bytes(Bytes::from(log_content), Some(options))
        .await;
    assert!(result.is_ok());

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    // Log files should be wrapped in code blocks
    assert!(markdown.contains("```"), "Log should be in code block");
}
