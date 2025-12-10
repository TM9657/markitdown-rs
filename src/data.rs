//! Data format converters (JSON, YAML, TOML, plain text).
//!
//! Converts structured data formats to readable markdown representation.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::ObjectStore;
use std::sync::Arc;

use crate::error::MarkitdownError;
use crate::model::{ContentBlock, ConversionOptions, Document, DocumentConverter, Page};

/// JSON to Markdown converter
pub struct JsonConverter;

impl JsonConverter {
    fn convert_json(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);

        // Parse to validate and pretty-print
        let value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| MarkitdownError::ParseError(format!("JSON parse error: {}", e)))?;

        let pretty = serde_json::to_string_pretty(&value)
            .map_err(|e| MarkitdownError::ParseError(format!("JSON format error: {}", e)))?;

        let mut document = Document::new();
        let mut page = Page::new(1);

        // Wrap in code block
        let markdown = format!("```json\n{}\n```", pretty);
        page.add_content(ContentBlock::Markdown(markdown));

        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for JsonConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        _options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        Self::convert_json(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".json"]
    }
}

/// YAML to Markdown converter
pub struct YamlConverter;

impl YamlConverter {
    fn convert_yaml(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);

        // Parse to validate
        let value: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| MarkitdownError::ParseError(format!("YAML parse error: {}", e)))?;

        let pretty = serde_yaml::to_string(&value)
            .map_err(|e| MarkitdownError::ParseError(format!("YAML format error: {}", e)))?;

        let mut document = Document::new();
        let mut page = Page::new(1);

        // Wrap in code block
        let markdown = format!("```yaml\n{}\n```", pretty.trim());
        page.add_content(ContentBlock::Markdown(markdown));

        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for YamlConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        _options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        Self::convert_yaml(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".yaml", ".yml"]
    }
}

/// TOML to Markdown converter
pub struct TomlConverter;

impl TomlConverter {
    fn convert_toml(bytes: &[u8]) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes);

        // Parse to validate
        let value: toml::Value = toml::from_str(&content)
            .map_err(|e| MarkitdownError::ParseError(format!("TOML parse error: {}", e)))?;

        let pretty = toml::to_string_pretty(&value)
            .map_err(|e| MarkitdownError::ParseError(format!("TOML format error: {}", e)))?;

        let mut document = Document::new();
        let mut page = Page::new(1);

        // Wrap in code block
        let markdown = format!("```toml\n{}\n```", pretty.trim());
        page.add_content(ContentBlock::Markdown(markdown));

        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for TomlConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        _options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        Self::convert_toml(&bytes)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".toml"]
    }
}

/// Plain text and Markdown passthrough converter
pub struct TextConverter;

impl TextConverter {
    fn convert_text(bytes: &[u8], is_markdown: bool) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes).to_string();

        let mut document = Document::new();
        let mut page = Page::new(1);

        if is_markdown {
            page.add_content(ContentBlock::Markdown(content));
        } else {
            page.add_content(ContentBlock::Text(content));
        }

        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for TextConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let is_markdown = options
            .as_ref()
            .and_then(|o| o.file_extension.as_ref())
            .map(|ext| ext == ".md" || ext == ".markdown")
            .unwrap_or(false);

        Self::convert_text(&bytes, is_markdown)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".txt", ".text", ".md", ".markdown", ".log"]
    }
}

/// Source code to Markdown converter (with syntax highlighting hints)
pub struct CodeConverter;

impl CodeConverter {
    fn get_language(extension: &str) -> &'static str {
        match extension.to_lowercase().as_str() {
            ".rs" => "rust",
            ".py" => "python",
            ".js" => "javascript",
            ".ts" => "typescript",
            ".jsx" => "jsx",
            ".tsx" => "tsx",
            ".go" => "go",
            ".java" => "java",
            ".c" => "c",
            ".cpp" | ".cc" | ".cxx" => "cpp",
            ".h" | ".hpp" => "cpp",
            ".cs" => "csharp",
            ".rb" => "ruby",
            ".php" => "php",
            ".swift" => "swift",
            ".kt" | ".kts" => "kotlin",
            ".scala" => "scala",
            ".sh" | ".bash" => "bash",
            ".zsh" => "zsh",
            ".ps1" => "powershell",
            ".sql" => "sql",
            ".r" => "r",
            ".lua" => "lua",
            ".pl" => "perl",
            ".ex" | ".exs" => "elixir",
            ".erl" => "erlang",
            ".hs" => "haskell",
            ".ml" | ".mli" => "ocaml",
            ".fs" | ".fsx" => "fsharp",
            ".clj" | ".cljs" => "clojure",
            ".vim" => "vim",
            ".dockerfile" => "dockerfile",
            ".tf" => "terraform",
            ".proto" => "protobuf",
            ".graphql" | ".gql" => "graphql",
            ".css" => "css",
            ".scss" | ".sass" => "scss",
            ".less" => "less",
            ".vue" => "vue",
            ".svelte" => "svelte",
            _ => "",
        }
    }

    fn convert_code(bytes: &[u8], extension: &str) -> Result<Document, MarkitdownError> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let language = Self::get_language(extension);

        let mut document = Document::new();
        let mut page = Page::new(1);

        // Wrap in code block with language hint
        let markdown = format!("```{}\n{}\n```", language, content);
        page.add_content(ContentBlock::Markdown(markdown));

        document.add_page(page);
        Ok(document)
    }
}

#[async_trait]
impl DocumentConverter for CodeConverter {
    async fn convert(
        &self,
        store: Arc<dyn ObjectStore>,
        path: &object_store::path::Path,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let result = store.get(path).await?;
        let bytes = result.bytes().await?;
        self.convert_bytes(bytes, options).await
    }

    async fn convert_bytes(
        &self,
        bytes: Bytes,
        options: Option<ConversionOptions>,
    ) -> Result<Document, MarkitdownError> {
        let extension = options
            .as_ref()
            .and_then(|o| o.file_extension.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("");

        Self::convert_code(&bytes, extension)
    }

    fn supported_extensions(&self) -> &[&str] {
        &[
            ".rs",
            ".py",
            ".js",
            ".ts",
            ".jsx",
            ".tsx",
            ".go",
            ".java",
            ".c",
            ".cpp",
            ".cc",
            ".cxx",
            ".h",
            ".hpp",
            ".cs",
            ".rb",
            ".php",
            ".swift",
            ".kt",
            ".kts",
            ".scala",
            ".sh",
            ".bash",
            ".zsh",
            ".ps1",
            ".sql",
            ".r",
            ".lua",
            ".pl",
            ".ex",
            ".exs",
            ".erl",
            ".hs",
            ".ml",
            ".mli",
            ".fs",
            ".fsx",
            ".clj",
            ".cljs",
            ".vim",
            ".dockerfile",
            ".tf",
            ".proto",
            ".graphql",
            ".gql",
            ".css",
            ".scss",
            ".sass",
            ".less",
            ".vue",
            ".svelte",
        ]
    }
}
