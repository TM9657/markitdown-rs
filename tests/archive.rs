//! Tests for archive format converters
//!
//! Test files sourced from Apache Tika test corpus:
//! https://github.com/apache/tika/tree/main/tika-parsers/tika-parsers-standard/tika-parsers-standard-modules

use bytes::Bytes;
use markitdown::{ConversionOptions, MarkItDown};
use std::fs;

const TEST_DIR: &str = "tests/test_files";

// Helper to get test file path
fn test_file(name: &str) -> String {
    format!("{}/{}", TEST_DIR, name)
}

// TAR tests (using testTAR_no_magic.tar from Apache Tika)
#[tokio::test]
async fn test_tar_conversion() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("testTAR_no_magic.tar"), None).await;
    assert!(result.is_ok(), "TAR conversion failed: {:?}", result.err());
    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(!markdown.is_empty(), "TAR should produce output");
}

#[tokio::test]
async fn test_tar_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes =
        fs::read(test_file("testTAR_no_magic.tar")).expect("Failed to read testTAR_no_magic.tar");
    let options = ConversionOptions::default().with_extension(".tar");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "TAR bytes conversion failed: {:?}",
        result.err()
    );
}

// ZIP archive tests (using testTika4424.zip from Apache Tika)
#[tokio::test]
async fn test_zip_with_multiple_files() {
    let md = MarkItDown::new();
    let result = md.convert(&test_file("testTika4424.zip"), None).await;
    assert!(result.is_ok(), "ZIP conversion failed: {:?}", result.err());
}

#[tokio::test]
async fn test_zip_bytes_conversion() {
    let md = MarkItDown::new();
    let bytes = fs::read(test_file("testTika4424.zip")).expect("Failed to read testTika4424.zip");
    let options = ConversionOptions::default().with_extension(".zip");
    let result = md.convert_bytes(Bytes::from(bytes), Some(options)).await;
    assert!(
        result.is_ok(),
        "ZIP bytes conversion failed: {:?}",
        result.err()
    );
}

// GZIP tests (create minimal in-memory test)
#[tokio::test]
async fn test_gzip_bytes_conversion() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let md = MarkItDown::new();

    // Create gzipped content in memory
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(b"Hello, this is gzipped text content for testing.")
        .unwrap();
    let compressed = encoder.finish().unwrap();

    let options = ConversionOptions::default().with_extension(".gz");
    let result = md
        .convert_bytes(Bytes::from(compressed), Some(options))
        .await;
    assert!(
        result.is_ok(),
        "GZIP bytes conversion failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(
        markdown.contains("Hello") || markdown.contains("gzipped"),
        "GZIP content should be readable"
    );
}

// BZIP2 tests (create minimal in-memory test)
#[tokio::test]
async fn test_bzip2_bytes_conversion() {
    use bzip2::write::BzEncoder;
    use bzip2::Compression;
    use std::io::Write;

    let md = MarkItDown::new();

    // Create bzip2 content in memory
    let mut encoder = BzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(b"Hello, this is bzip2 compressed text content for testing.")
        .unwrap();
    let compressed = encoder.finish().unwrap();

    let options = ConversionOptions::default().with_extension(".bz2");
    let result = md
        .convert_bytes(Bytes::from(compressed), Some(options))
        .await;
    assert!(
        result.is_ok(),
        "BZIP2 bytes conversion failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(
        markdown.contains("Hello") || markdown.contains("bzip2"),
        "BZIP2 content should be readable"
    );
}

// XZ tests (create minimal in-memory test)
#[tokio::test]
async fn test_xz_bytes_conversion() {
    use std::io::Write;
    use xz2::write::XzEncoder;

    let md = MarkItDown::new();

    // Create xz content in memory
    let mut encoder = XzEncoder::new(Vec::new(), 6);
    encoder
        .write_all(b"Hello, this is xz compressed text content for testing.")
        .unwrap();
    let compressed = encoder.finish().unwrap();

    let options = ConversionOptions::default().with_extension(".xz");
    let result = md
        .convert_bytes(Bytes::from(compressed), Some(options))
        .await;
    assert!(
        result.is_ok(),
        "XZ bytes conversion failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(
        markdown.contains("Hello") || markdown.contains("xz"),
        "XZ content should be readable"
    );
}

// ZSTD tests (create minimal in-memory test)
#[tokio::test]
async fn test_zstd_bytes_conversion() {
    let md = MarkItDown::new();

    // Create zstd content in memory
    let original = b"Hello, this is zstd compressed text content for testing.";
    let compressed = zstd::encode_all(&original[..], 3).unwrap();

    let options = ConversionOptions::default().with_extension(".zst");
    let result = md
        .convert_bytes(Bytes::from(compressed), Some(options))
        .await;
    assert!(
        result.is_ok(),
        "ZSTD bytes conversion failed: {:?}",
        result.err()
    );

    let doc = result.unwrap();
    let markdown = doc.to_markdown();
    assert!(
        markdown.contains("Hello") || markdown.contains("zstd"),
        "ZSTD content should be readable"
    );
}
