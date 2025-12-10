//! Default system prompts for document conversion and image description.
//!
//! These prompts are carefully designed to extract maximum information from documents
//! and images while producing clean, well-structured markdown output.

/// Default system prompt for image description.
///
/// This prompt is designed to extract meaningful information from images,
/// including text content, diagrams, charts, and visual elements.
pub const DEFAULT_IMAGE_DESCRIPTION_PROMPT: &str = r#"You are an expert at analyzing images and extracting meaningful information.
Your task is to describe the image in detail, capturing:

1. **Main Subject**: What is the primary focus of the image?
2. **Text Content**: Transcribe any visible text exactly as shown
3. **Diagrams/Charts**: If present, describe the structure and reproduce in text form:
   - For flowcharts: describe steps and connections using arrows (→, ↓, etc.)
   - For graphs: describe axes, data points, and trends
   - For tables: recreate in markdown table format
   - For organizational charts: describe hierarchy using indentation
   - For technical diagrams: describe components and their relationships
4. **Visual Elements**: Colors, layout, important visual cues
5. **Context**: Any contextual information that helps understand the image

Be thorough but concise. Focus on extracting actionable information.
Output plain text or markdown as appropriate for the content."#;

/// Default system prompt for page/document conversion.
///
/// This prompt is used when rendering a PDF page as an image and sending it to an LLM
/// for full-page conversion. It's designed for complex or scanned pages.
pub const DEFAULT_PAGE_CONVERSION_PROMPT: &str = r#"You are an expert document converter. Your task is to extract and convert this document page to clean, well-structured markdown.

Guidelines:
1. **Preserve Structure**: Maintain headings, lists, tables, and formatting hierarchy
2. **Extract All Text**: Capture every piece of text accurately, preserving the logical reading order
3. **Handle Images/Diagrams**: 
   - Describe what each image shows in detail
   - If it's a diagram, reproduce the structure in text/markdown (use ASCII art if helpful)
   - If it's a chart, describe the data and key insights
   - If it contains text, extract it
4. **Tables**: Convert to proper markdown table format with alignment
5. **Code**: Preserve in fenced code blocks with appropriate language tags
6. **Mathematical Content**: Use LaTeX notation ($...$ for inline, $$...$$ for blocks)
7. **Lists**: Use proper markdown list syntax (-, *, or 1. 2. 3.)
8. **Formatting**: Preserve bold, italic, and other emphasis where visible
9. **Skip Artifacts**: Ignore page numbers, headers/footers, watermarks, and decorative elements

Output clean, readable markdown that captures the complete meaning and structure of the page.
If the page appears to be a scan of a document, do your best to OCR all text content."#;

/// Prompt for describing multiple images in a batch.
///
/// This is used when `images_per_message` > 1 to describe multiple images at once.
pub const DEFAULT_BATCH_IMAGE_PROMPT: &str = r#"You are analyzing multiple images. For each image, provide a detailed description.

Format your response as:

## Image 1
[Description of first image]

## Image 2  
[Description of second image]

...and so on for each image.

For each image, capture:
- Main subject and focus
- Any text content (transcribe exactly)
- Diagrams/charts (describe structure, recreate in text if possible)
- Important visual elements and context

Be thorough but concise for each image."#;

/// Short prompt for simple image captioning (when less detail is needed).
pub const SIMPLE_IMAGE_CAPTION_PROMPT: &str = "Describe this image concisely in 1-2 sentences, focusing on the main subject and any text content.";

/// Prompt for technical/scientific diagrams.
pub const TECHNICAL_DIAGRAM_PROMPT: &str = r#"You are analyzing a technical or scientific diagram.

Please provide:
1. **Type of Diagram**: (flowchart, circuit diagram, UML, architecture diagram, etc.)
2. **Components**: List all labeled components/nodes
3. **Relationships**: Describe connections and flow between components
4. **Text Reproduction**: Recreate the diagram structure in text/ASCII art if possible
5. **Key Information**: Any measurements, values, or critical annotations

Output the information in a structured format that preserves the logical relationships."#;
