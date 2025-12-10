use clap::{command, Parser};
use std::fs;

use markitdown::{model::ConversionOptions, MarkItDown};

#[derive(Parser, Debug)]
#[command(name = "markitdown")]
struct Cli {
    #[arg(value_name = "FILE", index = 1)]
    input: String,

    #[arg(short, long)]
    output: Option<String>,

    #[arg(short, long)]
    format: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let output = match cli.output {
        Some(file) => file,
        None => "console".to_string(),
    };

    let format = match cli.format {
        Some(format) => {
            if format == "html" || format == "xlsx" || format == "pdf" {
                format
            } else {
                eprintln!(
                    "Warning: Unsupported format '{}'. Using auto-detection.",
                    format
                );
                "".to_string()
            }
        }
        None => "".to_string(),
    };

    let input_file = cli.input.trim().to_string();

    if !std::path::Path::new(&input_file).exists() {
        return Err(format!("Error: File '{}' not found", input_file).into());
    }

    let markitdown = MarkItDown::new();

    let options = if format.is_empty() {
        None
    } else {
        Some(ConversionOptions::default().with_extension(format!(".{}", format)))
    };

    // Use the async convert_file method for simplicity
    match markitdown.convert_file(&input_file).await {
        Ok(markdown) => {
            if output == "console" {
                println!("{}", &markdown);
            } else {
                fs::write(&output, &markdown)
                    .map_err(|e| format!("Failed to write to '{}': {}", output, e))?;
                eprintln!("Successfully converted to: {}", output);
            }
        }
        Err(e) => {
            // For more control, use the convert method with options
            if options.is_some() {
                let bytes = fs::read(&input_file)?;
                match markitdown
                    .convert_bytes(bytes::Bytes::from(bytes), options)
                    .await
                {
                    Ok(doc) => {
                        let markdown = doc.to_markdown();
                        if output == "console" {
                            println!("{}", &markdown);
                        } else {
                            fs::write(&output, &markdown)
                                .map_err(|e| format!("Failed to write to '{}': {}", output, e))?;
                            eprintln!("Successfully converted to: {}", output);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: Unable to convert file '{}'. {}", input_file, e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: Unable to convert file '{}'. {}", input_file, e);
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
