use bytes::Bytes;
use markitdown::{model::ConversionOptions, MarkItDown};

#[tokio::test]
async fn test_pptx_conversion() {
    let options = ConversionOptions {
        file_extension: Some(".pptx".to_string()),
        url: None,
        llm_client: None,
        extract_images: false,
    };

    let markitdown = MarkItDown::new();

    let result = markitdown.convert("tests/test_files/test.pptx", Some(options)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pptx_bytes_conversion() {
    let options = ConversionOptions {
        file_extension: Some(".pptx".to_string()),
        url: None,
        llm_client: None,
        extract_images: false,
    };

    let markitdown = MarkItDown::new();

    let result = markitdown.convert_bytes(Bytes::from_static(include_bytes!("./test_files/test.pptx")), Some(options)).await;
    assert!(result.is_ok());
}
