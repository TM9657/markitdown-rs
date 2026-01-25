//! Multi-page table detection and merging.
//!
//! This module provides functionality to detect tables that span multiple pages
//! and merge them into a single coherent table.

use regex::Regex;
use std::sync::LazyLock;

/// Represents a detected table fragment in markdown content
#[derive(Debug, Clone)]
pub struct TableFragment {
    /// The markdown content of the table
    pub content: String,
    /// Position in the original content (byte offset)
    pub start_pos: usize,
    /// End position in the original content (byte offset)
    pub end_pos: usize,
    /// Whether this table has a proper header row
    pub has_header: bool,
    /// Whether this table appears to be complete (has header and proper ending)
    pub is_complete: bool,
    /// Whether this table is at the start of the content (potential continuation)
    pub at_content_start: bool,
    /// Whether this table is at the end of the content (potential to-be-continued)
    pub at_content_end: bool,
    /// Number of columns detected
    pub column_count: usize,
    /// The header row if present
    pub headers: Option<Vec<String>>,
    /// Data rows (excluding header and separator)
    pub data_rows: Vec<Vec<String>>,
}

/// Regex patterns for table detection
static TABLE_ROW_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|(.+)\|\s*$").unwrap());

static TABLE_SEPARATOR_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|[\s:-]+\|[\s|:-]*$").unwrap());

/// Detect table fragments in markdown content
pub fn detect_table_fragments(content: &str) -> Vec<TableFragment> {
    let mut fragments = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return fragments;
    }

    let mut i = 0;
    while i < lines.len() {
        // Look for table start (a line with | characters)
        if TABLE_ROW_PATTERN.is_match(lines[i]) {
            let table_start_line = i;
            let mut table_end_line = i;
            let mut has_separator = false;
            let mut separator_line = None;

            // Find the extent of the table
            while table_end_line < lines.len() && TABLE_ROW_PATTERN.is_match(lines[table_end_line])
            {
                if TABLE_SEPARATOR_PATTERN.is_match(lines[table_end_line]) {
                    has_separator = true;
                    separator_line = Some(table_end_line);
                }
                table_end_line += 1;
            }

            // Calculate byte positions for string slicing
            // Each line in lines[] has had its \n stripped, so we need to add 1 byte for each newline
            // EXCEPT for the last line if the content doesn't end with \n
            // Using len() is correct here since we need byte positions for str slicing
            let content_ends_with_newline = content.ends_with('\n');

            let start_pos: usize = lines[..table_start_line]
                .iter()
                .enumerate()
                .map(|(idx, l)| {
                    // Add 1 byte for newline unless it's the last line of content without trailing newline
                    if idx == lines.len() - 1 && !content_ends_with_newline {
                        l.len()
                    } else {
                        l.len() + 1 // +1 for '\n' which is always 1 byte
                    }
                })
                .sum();

            let end_pos: usize = lines[..table_end_line]
                .iter()
                .enumerate()
                .map(|(idx, l)| {
                    // Add 1 byte for newline unless it's the last line of content without trailing newline
                    if idx == lines.len() - 1 && !content_ends_with_newline {
                        l.len()
                    } else {
                        l.len() + 1 // +1 for '\n' which is always 1 byte
                    }
                })
                .sum();

            // Verify that calculated positions are valid UTF-8 boundaries
            // This should always be true since we're summing line lengths + newlines,
            // which aligns with line boundaries in the original string
            debug_assert!(
                content.is_char_boundary(start_pos),
                "start_pos {} is not a char boundary",
                start_pos
            );
            debug_assert!(
                content.is_char_boundary(end_pos.min(content.len())),
                "end_pos {} is not a char boundary",
                end_pos
            );

            // Parse the table structure
            let table_lines = &lines[table_start_line..table_end_line];
            let (headers, data_rows, column_count) =
                parse_table_structure(table_lines, separator_line.map(|l| l - table_start_line));

            // Determine if table is at content boundaries
            let at_content_start = table_start_line == 0
                || lines[..table_start_line]
                    .iter()
                    .all(|l| l.trim().is_empty());

            let at_content_end = table_end_line >= lines.len()
                || lines[table_end_line..].iter().all(|l| l.trim().is_empty());

            // A table is complete if it has a header separator
            let is_complete = has_separator;
            let has_header = separator_line.is_some() && separator_line.unwrap() > table_start_line;

            let table_content = table_lines.join("\n");

            fragments.push(TableFragment {
                content: table_content,
                start_pos,
                end_pos,
                has_header,
                is_complete,
                at_content_start,
                at_content_end,
                column_count,
                headers,
                data_rows,
            });

            i = table_end_line;
        } else {
            i += 1;
        }
    }

    fragments
}

/// Parse table structure into headers and data rows
fn parse_table_structure(
    lines: &[&str],
    separator_idx: Option<usize>,
) -> (Option<Vec<String>>, Vec<Vec<String>>, usize) {
    let mut headers = None;
    let mut data_rows = Vec::new();
    let mut column_count = 0;

    for (i, line) in lines.iter().enumerate() {
        // Skip separator line
        if TABLE_SEPARATOR_PATTERN.is_match(line) {
            continue;
        }

        let cells = parse_table_row(line);
        if cells.is_empty() {
            continue;
        }

        column_count = column_count.max(cells.len());

        // First row before separator is header
        if separator_idx.is_some() && i == 0 {
            headers = Some(cells);
        } else if separator_idx.is_none() || i > separator_idx.unwrap() {
            data_rows.push(cells);
        } else if i > 0 && i < separator_idx.unwrap_or(usize::MAX) {
            // Rows between first row and separator (unusual but handle it)
            data_rows.push(cells);
        }
    }

    (headers, data_rows, column_count)
}

/// Parse a single table row into cells
fn parse_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return Vec::new();
    }

    // Remove leading and trailing |
    let inner = &trimmed[1..trimmed.len() - 1];

    inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// Check if two table fragments can be merged (appear to be continuation)
pub fn can_merge_tables(first: &TableFragment, second: &TableFragment) -> bool {
    // First table should be at the end of its content
    if !first.at_content_end {
        return false;
    }

    // Second table should be at the start of its content
    if !second.at_content_start {
        return false;
    }

    // Column counts should match (with some tolerance)
    if first.column_count != second.column_count
        && first.column_count > 0
        && second.column_count > 0
    {
        return false;
    }

    // If second has no header, it's likely a continuation
    if !second.has_header {
        return true;
    }

    // If both have headers, check if they match (same table split)
    if let (Some(h1), Some(h2)) = (&first.headers, &second.headers) {
        // Headers are identical - it's a continuation with repeated header
        if h1 == h2 {
            return true;
        }
    }

    // Default: don't merge if second has a different header
    false
}

/// Merge two table fragments into one
pub fn merge_table_fragments(first: &TableFragment, second: &TableFragment) -> TableFragment {
    let mut merged_data_rows = first.data_rows.clone();

    // The second table's data_rows already excludes the header row (it's in second.headers)
    // So we can simply append all data rows from the second table
    for row in second.data_rows.iter() {
        merged_data_rows.push(row.clone());
    }

    // Rebuild the markdown table
    let headers = first.headers.clone().or_else(|| second.headers.clone());
    let column_count = first.column_count.max(second.column_count);
    let content = build_markdown_table(&headers, &merged_data_rows, column_count);

    TableFragment {
        content,
        start_pos: 0, // Will be recalculated when replacing
        end_pos: 0,
        has_header: headers.is_some(),
        is_complete: true,
        at_content_start: first.at_content_start,
        at_content_end: second.at_content_end,
        column_count,
        headers,
        data_rows: merged_data_rows,
    }
}

/// Build a markdown table from headers and rows
fn build_markdown_table(
    headers: &Option<Vec<String>>,
    rows: &[Vec<String>],
    column_count: usize,
) -> String {
    let mut result = String::new();

    // Ensure we have a valid column count
    let col_count = if column_count > 0 {
        column_count
    } else if let Some(h) = headers {
        h.len()
    } else if let Some(first_row) = rows.first() {
        first_row.len()
    } else {
        return result;
    };

    // Header row
    if let Some(h) = headers {
        let mut padded = h.clone();
        while padded.len() < col_count {
            padded.push(String::new());
        }
        result.push_str("| ");
        result.push_str(&padded.join(" | "));
        result.push_str(" |\n");

        // Separator
        result.push_str("| ");
        result.push_str(&vec!["---"; col_count].join(" | "));
        result.push_str(" |\n");
    }

    // Data rows
    for row in rows {
        let mut padded = row.clone();
        while padded.len() < col_count {
            padded.push(String::new());
        }
        result.push_str("| ");
        result.push_str(&padded.join(" | "));
        result.push_str(" |\n");
    }

    result
}

/// Result of merging tables across pages
#[derive(Debug)]
pub struct MergedPageContent {
    /// The page number this content belongs to
    pub page_number: u32,
    /// The modified markdown content
    pub content: String,
    /// Whether any tables were merged from the next page
    pub merged_from_next: bool,
    /// Whether any tables were merged from the previous page
    pub merged_into_previous: bool,
}

/// Merge tables across multiple pages
///
/// Returns the modified content for each page, with multi-page tables merged.
/// Tables that span pages will be placed entirely on the first page where they start.
pub fn merge_tables_across_pages(pages: &[(u32, String)]) -> Vec<MergedPageContent> {
    if pages.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<MergedPageContent> = pages
        .iter()
        .map(|(num, content)| MergedPageContent {
            page_number: *num,
            content: content.clone(),
            merged_from_next: false,
            merged_into_previous: false,
        })
        .collect();

    // Process pairs of adjacent pages
    let mut i = 0;
    while i < results.len().saturating_sub(1) {
        let current_fragments = detect_table_fragments(&results[i].content);
        let next_fragments = detect_table_fragments(&results[i + 1].content);

        // Find tables at the end of current page that might continue
        if let Some(last_table) = current_fragments.last() {
            if last_table.at_content_end {
                // Find tables at the start of next page
                if let Some(first_table) = next_fragments.first() {
                    if can_merge_tables(last_table, first_table) {
                        // Merge the tables
                        let merged = merge_table_fragments(last_table, first_table);

                        // Update current page: replace last table with merged version
                        let before_table = &results[i].content[..last_table.start_pos];
                        let after_table = &results[i].content[last_table.end_pos..];
                        results[i].content =
                            format!("{}{}{}", before_table, merged.content, after_table);
                        results[i].merged_from_next = true;

                        // Update next page: remove the merged table
                        let before_table = &results[i + 1].content[..first_table.start_pos];
                        let after_table = &results[i + 1].content[first_table.end_pos..];
                        results[i + 1].content = format!("{}{}", before_table, after_table);
                        results[i + 1].merged_into_previous = true;

                        // Check if there are more tables to merge from this combined result
                        // (table might span more than 2 pages)
                        continue;
                    }
                }
            }
        }

        i += 1;
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_simple_table() {
        let content = r#"
Some text before

| Header 1 | Header 2 |
| --- | --- |
| Cell 1 | Cell 2 |
| Cell 3 | Cell 4 |

Some text after
"#;

        let fragments = detect_table_fragments(content);
        assert_eq!(fragments.len(), 1);
        assert!(fragments[0].has_header);
        assert!(fragments[0].is_complete);
        assert_eq!(fragments[0].column_count, 2);
        assert_eq!(
            fragments[0].headers,
            Some(vec!["Header 1".to_string(), "Header 2".to_string()])
        );
        assert_eq!(fragments[0].data_rows.len(), 2);
    }

    #[test]
    fn test_detect_table_without_header() {
        let content = r#"| Cell 1 | Cell 2 |
| Cell 3 | Cell 4 |
"#;

        let fragments = detect_table_fragments(content);
        assert_eq!(fragments.len(), 1);
        assert!(!fragments[0].has_header);
        assert!(!fragments[0].is_complete);
        assert!(fragments[0].at_content_start);
    }

    #[test]
    fn test_can_merge_continuation() {
        let first = TableFragment {
            content: "| A | B |\n| --- | --- |\n| 1 | 2 |".to_string(),
            start_pos: 0,
            end_pos: 0,
            has_header: true,
            is_complete: true,
            at_content_start: false,
            at_content_end: true,
            column_count: 2,
            headers: Some(vec!["A".to_string(), "B".to_string()]),
            data_rows: vec![vec!["1".to_string(), "2".to_string()]],
        };

        let second = TableFragment {
            content: "| 3 | 4 |\n| 5 | 6 |".to_string(),
            start_pos: 0,
            end_pos: 0,
            has_header: false,
            is_complete: false,
            at_content_start: true,
            at_content_end: false,
            column_count: 2,
            headers: None,
            data_rows: vec![
                vec!["3".to_string(), "4".to_string()],
                vec!["5".to_string(), "6".to_string()],
            ],
        };

        assert!(can_merge_tables(&first, &second));
    }

    #[test]
    fn test_merge_tables() {
        let first = TableFragment {
            content: String::new(),
            start_pos: 0,
            end_pos: 0,
            has_header: true,
            is_complete: true,
            at_content_start: false,
            at_content_end: true,
            column_count: 2,
            headers: Some(vec!["Name".to_string(), "Value".to_string()]),
            data_rows: vec![vec!["Row1".to_string(), "100".to_string()]],
        };

        let second = TableFragment {
            content: String::new(),
            start_pos: 0,
            end_pos: 0,
            has_header: false,
            is_complete: false,
            at_content_start: true,
            at_content_end: true,
            column_count: 2,
            headers: None,
            data_rows: vec![
                vec!["Row2".to_string(), "200".to_string()],
                vec!["Row3".to_string(), "300".to_string()],
            ],
        };

        let merged = merge_table_fragments(&first, &second);
        assert_eq!(merged.data_rows.len(), 3);
        assert!(merged.has_header);
        assert!(merged.content.contains("| Name | Value |"));
        assert!(merged.content.contains("| Row1 | 100 |"));
        assert!(merged.content.contains("| Row2 | 200 |"));
        assert!(merged.content.contains("| Row3 | 300 |"));
    }
}
