use markitdown::create_llm_client;
use rig::client::CompletionClient;
use rig::providers::openrouter;
use std::env;

#[tokio::test]
async fn test_llm_integration() {
    // 1. Load .env file if present
    let _ = dotenvy::dotenv();

    // 2. Check for required environment variables
    let api_key = env::var("OPENROUTER_API_KEY");
    let endpoint = env::var("OPENROUTER_ENDPOINT");
    let model = env::var("OPENROUTER_MODEL");

    if api_key.is_err() || endpoint.is_err() || model.is_err() {
        println!("Skipping LLM test: Missing OPENROUTER_API_KEY, OPENROUTER_ENDPOINT, or OPENROUTER_MODEL");
        return;
    }

    let api_key = api_key.unwrap();
    let endpoint = endpoint.unwrap();
    let model_name = model.unwrap();

    // Normalize to OpenRouter's expected base URL (usually https://openrouter.ai/api/v1)
    // Users may provide:
    // - https://openrouter.ai/api
    // - https://openrouter.ai/api/
    // - https://openrouter.ai/api/v1
    // - https://openrouter.ai/api/v1/
    // We accept any of the above and normalize.
    let endpoint = endpoint.trim_end_matches('/').to_string();
    let endpoint = if endpoint.ends_with("/api/v1") {
        endpoint
    } else if endpoint.ends_with("/api") {
        format!("{}/v1", endpoint)
    } else if endpoint.ends_with("/v1") {
        endpoint
    } else {
        // Best effort: append /v1 (covers custom OpenAI-compatible gateways)
        format!("{}/v1", endpoint)
    };

    println!("Running LLM test with model: {}", model_name);

    // 3. Create an OpenRouter client (do NOT use the OpenAI client).
    // We still honor OPENROUTER_ENDPOINT so you can point at a proxy/gateway.
    let client = openrouter::Client::builder(&api_key)
        .base_url(endpoint.as_str())
        .build();

    let model = client.completion_model(&model_name);
    
    // 4. Create LLM Wrapper
    let llm_client = create_llm_client(model);

    // 5. Test basic functionality: Describe a minimal 1x1 PNG image
    // Minimal 1x1 pixel transparent PNG
    let png_data: [u8; 67] = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // Signature
        0x00, 0x00, 0x00, 0x0D, // IHDR length
        0x49, 0x48, 0x44, 0x52, // IHDR chunk type
        0x00, 0x00, 0x00, 0x01, // Width: 1
        0x00, 0x00, 0x00, 0x01, // Height: 1
        0x08, // Bit depth: 8
        0x06, // Color type: Truecolor with alpha
        0x00, // Compression method
        0x00, // Filter method
        0x00, // Interlace method
        0x1F, 0x15, 0xC4, 0x89, // IHDR CRC
        0x00, 0x00, 0x00, 0x0A, // IDAT length
        0x49, 0x44, 0x41, 0x54, // IDAT chunk type
        0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, // Compressed data
        0x0D, 0x0A, 0x2D, 0xB4, // IDAT CRC
        0x00, 0x00, 0x00, 0x00, // IEND length
        0x49, 0x45, 0x4E, 0x44, // IEND chunk type
        0xAE, 0x42, 0x60, 0x82, // IEND CRC
    ];

    println!("Sending request to LLM...");
    let result = llm_client.describe_image(&png_data, "image/png").await;

    match result {
        Ok(description) => {
            println!("✓ LLM Description: {}", description);
            assert!(!description.is_empty(), "Description should not be empty");
        }
        Err(e) => panic!("LLM test failed: {}", e),
    }
}
