# markitdown-rs

markitdown-rs is a Rust library designed to facilitate the conversion of various document formats into markdown text. It is a Rust implementation of the original [markitdown](https://github.com/microsoft/markitdown) Python library.

## Features

### Document Formats

**Microsoft Office (Modern)**
- [x] Word (.docx, .dotx, .dotm)
- [x] Excel (.xlsx, .xltx, .xltm)
- [x] PowerPoint (.pptx, .potx, .potm)

**Microsoft Office (Legacy)**
- [x] Word 97-2003 (.doc)
- [x] Excel 97-2003 (.xls)
- [x] PowerPoint 97-2003 (.ppt)
- [x] Rich Text Format (.rtf)

**OpenDocument Format**
- [x] Text (.odt, .ott)
- [x] Spreadsheet (.ods, .ots)
- [x] Presentation (.odp, .otp)

**Apple iWork**
- [x] Pages (.pages)
- [x] Numbers (.numbers)
- [x] Keynote (.key)

**Other Document Formats**
- [x] PDF (.pdf)
  - **Intelligent fallback mechanism**: Automatically detects scanned PDFs, complex pages with diagrams, or pages with limited text and images
  - Uses text extraction by default for efficiency
  - Falls back to LLM-powered page rendering when:
    - Page has < 10 words (likely scanned)
    - Low alphanumeric ratio < 0.5 (OCR artifacts/garbage)
    - Unstructured content < 50 characters
    - Page contains images + < 350 words (provides full context to LLM)
  - Renders entire page as PNG for LLM processing when needed
- [x] EPUB (.epub)
- [x] Markdown (.md)

### Data Formats

- [x] CSV (.csv)
- [x] Excel spreadsheets (.xlsx, .xls)
- [x] SQLite databases (.sqlite, .db)

### Structured Data

- [x] XML (.xml)
- [x] RSS feeds (.rss, .atom)
- [x] HTML (.html, .htm)
- [x] Email (.eml, .msg)
- [x] vCard (.vcf)
- [x] iCalendar (.ics)
- [x] BibTeX (.bib)

### Archive Formats

- [x] ZIP (.zip)
- [x] TAR (.tar, .tar.gz, .tar.bz2, .tar.xz)
- [x] GZIP (.gz)
- [x] BZIP2 (.bz2)
- [x] XZ (.xz)
- [x] ZSTD (.zst)
- [x] 7-Zip (.7z)

### Media

- [x] Images (.jpg, .png, .gif, .bmp, .tiff, .webp)
  - With LLM integration for intelligent image descriptions
- [ ] Audio (planned)

### Other

- [x] Plain text (.txt)
- [x] Log files (.log)

> **Note:** All formats support both file path and in-memory bytes conversion.

## Usage

### Command-Line

#### Installation

```
cargo install markitdown
```

#### Convert a File

```
markitdown path-to-file.pdf
```

Or use -o to specify the output file:

```
markitdown path-to-file.pdf -o document.md
```

Supported formats include Office documents (.docx, .xlsx, .pptx), legacy Office (.doc, .xls, .ppt), OpenDocument (.odt, .ods), Apple iWork (.pages, .numbers, .key), PDFs, EPUB, images, archives, and more. See the full list above.

### Rust API

#### Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
markitdown = "0.1.10"
```

#### Initialize MarkItDown

```rust
use markitdown::MarkItDown;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let md = MarkItDown::new();
    Ok(())
}
```

#### Convert a File

```rust
use markitdown::{ConversionOptions, MarkItDown};
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let md = MarkItDown::new();
    
    // Create a local file system object store
    let store = Arc::new(LocalFileSystem::new());
    
    // Convert file path string to ObjectStore Path
    let path = ObjectPath::from("path/to/file.xlsx");

    // Basic conversion - file type is auto-detected from extension
    let result = md.convert_with_store(store.clone(), &path, None).await?;
    println!("Converted Text: {}", result.to_markdown());

    // Convert legacy Office formats
    let doc_path = ObjectPath::from("document.doc");
    let result = md.convert_with_store(store.clone(), &doc_path, None).await?;

    // Convert archives (extracts and converts contents)
    let zip_path = ObjectPath::from("archive.zip");
    let result = md.convert_with_store(store.clone(), &zip_path, None).await?;

    // Or explicitly specify options
    let options = ConversionOptions::default()
        .with_extension(".xlsx")
        .with_extract_images(true);

    let result = md.convert_with_store(store, &path, Some(options)).await?;
    
    Ok(())
}
```

> **Important:** The library uses `object_store` for file operations, not plain file paths. You must:
> 1. Create an `ObjectStore` implementation (like `LocalFileSystem` for local files)
> 2. Convert file path strings to `object_store::path::Path` using `Path::from()`
> 3. Use `convert_with_store()` method with the store and path
>
> For convenience, there's also a `convert()` method that accepts string paths and uses `LocalFileSystem` internally.

#### Convert with LLM for Image Descriptions

```rust
use markitdown::{ConversionOptions, MarkItDown, create_llm_client};
use rig::providers::openai;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let md = MarkItDown::new();
    
    // Create an LLM client using any rig-core compatible provider
    // OpenAI example:
    let openai_client = openai::Client::from_env();
    let model = openai_client.completion_model("gpt-4o");
    let llm = create_llm_client(model);
    
    // Google Gemini example:
    // let gemini_client = gemini::Client::from_env();
    // let model = gemini_client.completion_model("gemini-2.0-flash");
    // let llm = create_llm_client(model);
    
    // Anthropic Claude example:
    // let anthropic_client = anthropic::Client::from_env();
    // let model = anthropic_client.completion_model("claude-sonnet-4-20250514");
    // let llm = create_llm_client(model);
    
    // Cohere example with custom endpoint:
    // let api_key = std::env::var("COHERE_API_KEY")?;
    // let mut builder = rig::providers::cohere::Client::builder(&api_key);
    // if let Some(endpoint) = custom_endpoint {
    //     builder = builder.base_url(endpoint);
    // }
    // let client = builder.build();
    // let model = client.completion_model("command-r-plus");
    // let llm = create_llm_client(model);

    let options = ConversionOptions::default()
        .with_extension(".jpg")
        .with_llm(llm);

    let result = md.convert("path/to/image.jpg", Some(options)).await?;
    println!("Image description: {}", result.to_markdown());
    
    Ok(())
}
```

**Supported LLM Providers** (via rig-core):
- OpenAI (GPT-4, GPT-4o, etc.)
- Google Gemini (gemini-2.0-flash, gemini-pro, etc.)
- Anthropic Claude (claude-sonnet, claude-opus, etc.)
- Cohere (command-r-plus, etc.)
- Any custom provider implementing `CompletionModel`

#### Convert from Bytes

```rust
use markitdown::{ConversionOptions, MarkItDown};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let md = MarkItDown::new();
    
    let file_bytes = std::fs::read("path/to/file.pdf")?;

    // Auto-detect file type from bytes
    let result = md.convert_bytes(Bytes::from(file_bytes.clone()), None).await?;
    println!("Converted: {}", result.to_markdown());

    // Or specify options explicitly
    let options = ConversionOptions::default()
        .with_extension(".pdf");

    let result = md.convert_bytes(Bytes::from(file_bytes), Some(options)).await?;
    
    Ok(())
}
```

## Recent Improvements

### Format Expansion
- **40+ new formats** including legacy Office (.doc, .xls, .ppt), OpenDocument (.odt, .ods, .odp), Apple iWork (.pages, .numbers, .key)
- **Archive support** for ZIP, TAR, GZIP, BZIP2, XZ, ZSTD, and 7-Zip with automatic content extraction
- **Additional formats**: EPUB, vCard, iCalendar, BibTeX, log files, SQLite databases, email files

### Performance & Reliability
- **Static compilation** for compression libraries (bzip2, xz2) for better portability
- **Improved file detection** - prioritizes file extension over magic byte detection for legacy formats
- **Template support** for Office formats (.dotx, .potx, .xltx)
- **LLM flexibility** - works with any rig-core compatible model (OpenAI, Gemini, Claude, Cohere, custom providers)

### Testing
- **Comprehensive test suite** using real-world files from [Apache Tika test corpus](https://github.com/apache/tika)
- Tests for all supported formats with both file and bytes conversion
- In-memory test generation for compression formats

#### Register a Custom Converter

You can extend MarkItDown by implementing the `DocumentConverter` trait for your custom converters and registering them:

```rust
use markitdown::{DocumentConverter, Document, ConversionOptions, MarkItDown};
use markitdown::error::MarkitdownError;
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use object_store::ObjectStore;

struct MyCustomConverter;

#[async_trait]
impl DocumentConverter for MyCustomConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // Implement file conversion logic
        todo!()
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        // Implement bytes conversion logic
        todo!()
    }
    
    fn supported_extensions(&self) -> &[&str] {
        &[".custom"]
    }
}

let mut md = MarkItDown::new();
md.register_converter(Box::new(MyCustomConverter));
```

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Acknowledgments

- Original Python implementation: [microsoft/markitdown](https://github.com/microsoft/markitdown)
- Test files from: [Apache Tika test corpus](https://github.com/apache/tika)

## License

MarkItDown is licensed under the MIT License. See `LICENSE` for more details.
