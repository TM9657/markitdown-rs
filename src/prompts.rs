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
pub const DEFAULT_PAGE_CONVERSION_PROMPT: &str = r#"Convert this document page to markdown. Output ONLY the converted content.

Rules:
- Extract all text exactly as written, preserving the original language
- Use proper heading levels (#, ##, ###) for titles
- Format lists, tables, and quotes appropriately  
- For images: add *[Image: brief description]*
- For charts/diagrams: describe data or structure briefly
- Keep citations and references intact
- NO commentary, explanations, or notes about your process
- NO markdown code block wrappers around the output
- Do NOT repeat content - each element should appear only once"#;

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
