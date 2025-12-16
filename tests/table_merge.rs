//! Multi-page table merging tests
//!
//! These tests verify the table merging functionality that combines
//! tables spanning multiple pages into single coherent tables.

use markitdown::table_merge::{
    can_merge_tables, detect_table_fragments, merge_table_fragments, merge_tables_across_pages,
};

// ============================================================================
// Table Detection Tests
// ============================================================================

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
    assert_eq!(fragments.len(), 1, "Should detect exactly one table");
    assert!(fragments[0].has_header, "Table should have a header");
    assert!(fragments[0].is_complete, "Table should be complete");
    assert_eq!(fragments[0].column_count, 2, "Table should have 2 columns");
    assert_eq!(
        fragments[0].headers,
        Some(vec!["Header 1".to_string(), "Header 2".to_string()])
    );
    assert_eq!(
        fragments[0].data_rows.len(),
        2,
        "Table should have 2 data rows"
    );
}

#[test]
fn test_detect_table_without_header() {
    let content = r#"| Cell 1 | Cell 2 |
| Cell 3 | Cell 4 |
"#;

    let fragments = detect_table_fragments(content);
    assert_eq!(fragments.len(), 1, "Should detect exactly one table");
    assert!(!fragments[0].has_header, "Table should not have a header");
    assert!(
        fragments[0].at_content_start,
        "Table should be at content start"
    );
}

#[test]
fn test_detect_multiple_tables() {
    let content = r#"
| A | B |
| --- | --- |
| 1 | 2 |

Some text between tables

| X | Y | Z |
| --- | --- | --- |
| a | b | c |
"#;

    let fragments = detect_table_fragments(content);
    assert_eq!(fragments.len(), 2, "Should detect two tables");
    assert_eq!(
        fragments[0].column_count, 2,
        "First table should have 2 columns"
    );
    assert_eq!(
        fragments[1].column_count, 3,
        "Second table should have 3 columns"
    );
}

#[test]
fn test_detect_table_at_boundaries() {
    // Table at end of content
    let content_end = r#"Some text

| A | B |
| --- | --- |
| 1 | 2 |"#;

    let fragments = detect_table_fragments(content_end);
    assert_eq!(fragments.len(), 1);
    assert!(
        fragments[0].at_content_end,
        "Table should be at content end"
    );

    // Table at start of content
    let content_start = r#"| A | B |
| --- | --- |
| 1 | 2 |

Some text"#;

    let fragments = detect_table_fragments(content_start);
    assert_eq!(fragments.len(), 1);
    assert!(
        fragments[0].at_content_start,
        "Table should be at content start"
    );
}

// ============================================================================
// Table Merging Logic Tests
// ============================================================================

#[test]
fn test_can_merge_continuation_table() {
    // First table at end of page
    let first_content = r#"Some text

| Name | Age |
| --- | --- |
| Alice | 30 |"#;

    // Second table at start of next page (no header = continuation)
    let second_content = r#"| Bob | 25 |
| Charlie | 35 |

More text"#;

    let first_fragments = detect_table_fragments(first_content);
    let second_fragments = detect_table_fragments(second_content);

    assert_eq!(first_fragments.len(), 1);
    assert_eq!(second_fragments.len(), 1);
    assert!(
        can_merge_tables(&first_fragments[0], &second_fragments[0]),
        "Should be able to merge continuation table"
    );
}

#[test]
fn test_cannot_merge_different_column_counts() {
    let first_content = r#"| A | B |
| --- | --- |
| 1 | 2 |"#;

    let second_content = r#"| X | Y | Z |
| 4 | 5 | 6 |"#;

    let first_fragments = detect_table_fragments(first_content);
    let second_fragments = detect_table_fragments(second_content);

    assert!(
        !can_merge_tables(&first_fragments[0], &second_fragments[0]),
        "Should not merge tables with different column counts"
    );
}

#[test]
fn test_merge_tables_with_same_header() {
    // First table
    let first_content = r#"| Name | Value |
| --- | --- |
| A | 1 |"#;

    // Second table with repeated header (common in PDF table continuation)
    let second_content = r#"| Name | Value |
| --- | --- |
| B | 2 |"#;

    let first_fragments = detect_table_fragments(first_content);
    let second_fragments = detect_table_fragments(second_content);

    assert!(can_merge_tables(&first_fragments[0], &second_fragments[0]));

    let merged = merge_table_fragments(&first_fragments[0], &second_fragments[0]);
    assert_eq!(
        merged.headers,
        Some(vec!["Name".to_string(), "Value".to_string()])
    );
    // Should have 2 data rows (one from each table, duplicate header not added as data)
    assert_eq!(merged.data_rows.len(), 2);
}

// ============================================================================
// Multi-Page Merge Tests
// ============================================================================

#[test]
fn test_merge_tables_across_two_pages() {
    let page1 = r#"# Page 1

| Product | Price |
| --- | --- |
| Apple | 1.00 |"#;

    let page2 = r#"| Banana | 0.50 |
| Orange | 0.75 |

End of table"#;

    let pages = vec![(1, page1.to_string()), (2, page2.to_string())];

    let merged = merge_tables_across_pages(&pages);

    assert_eq!(merged.len(), 2);
    assert!(
        merged[0].merged_from_next,
        "Page 1 should have merged from next"
    );
    assert!(
        merged[1].merged_into_previous,
        "Page 2 should be merged into previous"
    );

    // Check merged content on page 1
    let page1_content = &merged[0].content;
    assert!(
        page1_content.contains("Apple"),
        "Merged content should contain Apple"
    );
    assert!(
        page1_content.contains("Banana"),
        "Merged content should contain Banana"
    );
    assert!(
        page1_content.contains("Orange"),
        "Merged content should contain Orange"
    );

    // Page 2 should have the table removed
    let page2_content = &merged[1].content;
    assert!(
        !page2_content.contains("Banana") || page2_content.contains("End of table"),
        "Page 2 should have table removed or only have remaining content"
    );
}

#[test]
fn test_merge_tables_across_three_pages() {
    let page1 = r#"| Col1 | Col2 |
| --- | --- |
| A | 1 |"#;

    let page2 = r#"| B | 2 |
| C | 3 |"#;

    let page3 = r#"| D | 4 |
| E | 5 |

Footer text"#;

    let pages = vec![
        (1, page1.to_string()),
        (2, page2.to_string()),
        (3, page3.to_string()),
    ];

    let merged = merge_tables_across_pages(&pages);

    assert_eq!(merged.len(), 3);

    // First page should contain all data
    let page1_content = &merged[0].content;
    assert!(page1_content.contains("A"));
    // Due to iterative merging, all rows should eventually be on page 1
}

#[test]
fn test_no_merge_when_tables_not_at_boundaries() {
    let page1 = r#"Some text

| A | B |
| --- | --- |
| 1 | 2 |

More text after table"#;

    let page2 = r#"Text before table

| X | Y |
| --- | --- |
| 3 | 4 |"#;

    let pages = vec![(1, page1.to_string()), (2, page2.to_string())];

    let merged = merge_tables_across_pages(&pages);

    assert!(
        !merged[0].merged_from_next,
        "Should not merge when table not at boundary"
    );
    assert!(
        !merged[1].merged_into_previous,
        "Should not merge when table not at boundary"
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_pages() {
    let pages: Vec<(u32, String)> = vec![];
    let merged = merge_tables_across_pages(&pages);
    assert!(merged.is_empty());
}

#[test]
fn test_single_page() {
    let page = r#"| A | B |
| --- | --- |
| 1 | 2 |"#;

    let pages = vec![(1, page.to_string())];
    let merged = merge_tables_across_pages(&pages);

    assert_eq!(merged.len(), 1);
    assert!(!merged[0].merged_from_next);
    assert!(!merged[0].merged_into_previous);
}

#[test]
fn test_pages_without_tables() {
    let page1 = "Just some text on page 1";
    let page2 = "Just some text on page 2";

    let pages = vec![(1, page1.to_string()), (2, page2.to_string())];
    let merged = merge_tables_across_pages(&pages);

    assert_eq!(merged.len(), 2);
    assert!(!merged[0].merged_from_next);
    assert!(!merged[1].merged_into_previous);
}

#[test]
fn test_table_with_empty_cells() {
    let content = r#"| A | B | C |
| --- | --- | --- |
| 1 |  | 3 |
|  | 2 |  |"#;

    let fragments = detect_table_fragments(content);
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].column_count, 3);
}

// ============================================================================
// Real-World Scenarios
// ============================================================================

#[test]
fn test_financial_table_continuation() {
    // Simulates a financial report table split across pages
    let page1 = r#"# Q4 2024 Financial Report

| Quarter | Revenue | Expenses | Profit |
| --- | --- | --- | --- |
| Q1 | $100M | $80M | $20M |
| Q2 | $120M | $90M | $30M |"#;

    let page2 = r#"| Q3 | $140M | $100M | $40M |
| Q4 | $160M | $110M | $50M |

Total annual revenue: $520M"#;

    let pages = vec![(1, page1.to_string()), (2, page2.to_string())];
    let merged = merge_tables_across_pages(&pages);

    let page1_content = &merged[0].content;

    // All quarters should be in the merged table
    assert!(page1_content.contains("Q1"));
    assert!(page1_content.contains("Q2"));
    assert!(page1_content.contains("Q3"));
    assert!(page1_content.contains("Q4"));
}

#[test]
fn test_employee_roster_continuation() {
    // Simulates an employee roster split across pages
    let page1 = r#"| Employee | Department | Start Date |
| --- | --- | --- |
| Alice Smith | Engineering | 2020-01-15 |
| Bob Jones | Marketing | 2019-06-01 |"#;

    // Continuation without header
    let page2 = r#"| Carol White | Sales | 2021-03-10 |
| David Brown | Engineering | 2022-08-20 |"#;

    let pages = vec![(1, page1.to_string()), (2, page2.to_string())];
    let merged = merge_tables_across_pages(&pages);

    assert!(merged[0].merged_from_next);

    let content = &merged[0].content;
    assert!(content.contains("Alice"));
    assert!(content.contains("Carol"));
    assert!(content.contains("David"));
}
