use bytes::Bytes;
use markitdown::{model::ConversionOptions, MarkItDown};

#[tokio::test]
async fn test_excel_conversion() {
    let options = ConversionOptions {
        file_extension: Some(".xlsx".to_string()),
        url: None,
        llm_client: None,
        extract_images: false,
    };

    let markitdown = MarkItDown::new();

    let result = markitdown
        .convert("tests/test_files/test.xlsx", Some(options))
        .await;
    assert!(result.is_ok());
    // write_to_file(&result.unwrap().to_markdown());
}

#[tokio::test]
async fn test_excel_bytes_conversion() {
    let options = ConversionOptions {
        file_extension: Some(".xlsx".to_string()),
        url: None,
        llm_client: None,
        extract_images: false,
    };

    let markitdown = MarkItDown::new();

    let result = markitdown
        .convert_bytes(
            Bytes::from_static(include_bytes!("./test_files/test.xlsx")),
            Some(options),
        )
        .await;
    assert!(result.is_ok());
}

fn write_to_file(content: &str) {
    use std::io::Write;
    let mut file = std::fs::File::create("test.md").unwrap();
    file.write_all(content.as_bytes()).unwrap();
}
