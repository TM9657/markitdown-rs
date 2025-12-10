use bytes::Bytes;
use markitdown::{model::ConversionOptions, MarkItDown};

#[tokio::test]
async fn test_html_conversion() {
    let options = ConversionOptions {
        file_extension: Some(".html".to_string()),
        url: None,
        llm_client: None,
        extract_images: true,
    };

    let markitdown = MarkItDown::new();

    let result = markitdown
        .convert("tests/test_files/test_blog.html", Some(options))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_html_bytes_conversion() {
    let options = ConversionOptions {
        file_extension: Some(".html".to_string()),
        url: None,
        llm_client: None,
        extract_images: true,
    };

    let markitdown = MarkItDown::new();

    let result = markitdown
        .convert_bytes(
            Bytes::from_static(include_bytes!("./test_files/test_blog.html")),
            Some(options),
        )
        .await;
    assert!(result.is_ok());
}
