# Format Coverage and Tests

Summary of supported converters, extensions, and where coverage lives in tests. All paths are relative to the repo root.

## Converter Matrix

| Category | Format | Converter | Key Extensions | Test File | Status / Notes |
| --- | --- | --- | --- | --- | --- |
| Archives | Zip/Tar/Gzip/Bzip2/XZ/Zstd | `ArchiveConverter` | .zip .tar.gz .tgz .gz .bz2 .xz .zst | `tests/archive.rs` | ✅
| Calendars | iCalendar | `ICalendarConverter` | .ics | `tests/calendar.rs` | ✅
| Contacts | vCard | `VCardConverter` | .vcf | `tests/vcard.rs` | ✅
| Data | JSON | `JsonConverter` | .json | `tests/json.rs` | ✅
| Data | YAML | `YamlConverter` | .yaml .yml | `tests/yaml.rs` | ✅
| Data | TOML | `TomlConverter` | .toml | (covered indirectly via data converter) | ⚪
| Data | Plain text | `TextConverter` | .txt | `tests/text.rs` | ✅
| Data | Code snippet passthrough | `CodeConverter` | (no ext required) | (converter utility) | ⚪
| Documents | Markdown | `MarkdownConverter` | .md .markdown | `tests/markdown.rs` | ✅
| Documents | HTML | `HtmlConverter` | .html .htm | `tests/html.rs` | ✅
| Documents | PDF | `PdfConverter` | .pdf | `tests/pdf.rs` (1 ignored) | ✅
| Documents | RTF | `RtfConverter` | .rtf | `tests/rtf.rs` | ✅
| Documents | DocBook | `DocBookConverter` | .docbook .docbook4 .docbook5 .dbk | `tests/docbook.rs` | ✅
| Documents | OPML | `OpmlConverter` | .opml | `tests/opml.rs` | ✅
| Documents | FictionBook | `FictionBookConverter` | .fb2 .xml | `tests/fictionbook.rs` | ✅
| Documents | Org-mode | `OrgModeConverter` | .org | `tests/orgmode.rs` | ✅
| Documents | reStructuredText | `RstConverter` | .rst | `tests/rst.rs` | ✅
| Documents | LaTeX | `LatexConverter` | .tex .latex | `tests/latex.rs` | ✅
| Documents | Typst | `TypstConverter` | .typ | `tests/typst.rs` | ✅
| Documents | Log files | `LogConverter` | .log | `tests/bibtex_log.rs` (shared) | ✅
| Documents | Bibliography | `BibtexConverter` | .bib | `tests/bibtex_log.rs` | ✅
| Ebooks | EPUB | `EpubConverter` | .epub | `tests/epub.rs` | ✅
| Ebooks | FB2 | `FictionBookConverter` | .fb2 | `tests/fictionbook.rs` | ✅
| Ebooks | DocBook | `DocBookConverter` | .docbook* .dbk | `tests/docbook.rs` | ✅
| Email | EML/MSG | `EmailConverter` | .eml .msg | `tests/email.rs` | ✅
| Feeds | RSS/Atom | `RssConverter` | .xml (rss/atom) | `tests/rss.rs` | ✅
| Images | Raster | `ImageConverter` | .png .jpg .jpeg .bmp .gif .tiff .webp | `tests/image.rs` | ✅
| Legacy Office | Word 97-2003 | `DocConverter` | .doc | `tests/legacy_office.rs` | ✅
| Legacy Office | Excel 97-2003 | `XlsConverter` | .xls | `tests/legacy_office.rs` | ✅
| Legacy Office | PowerPoint 97-2003 | `PptConverter` | .ppt | `tests/legacy_office.rs` | ✅
| Legacy Office Templates | DOTX/XLT/XLTX/POTX | `DotxConverter` `XltxConverter` `PotxConverter` | .dotx .xlt .xltx .potx | `tests/legacy_office.rs` | ✅
| Modern Office | Word (docx) | `DocxConverter` | .docx | `tests/docx.rs` | ✅
| Modern Office | Excel (xlsx) | `ExcelConverter` | .xlsx | `tests/excel.rs` | ✅
| Modern Office | PowerPoint (pptx) | `PptxConverter` | .pptx | `tests/pptx.rs` | ✅
| Modern Office (flat) | ODT/ODS/ODP | `OdtConverter` `OdsConverter` `OdpConverter` | .odt .ods .odp | `tests/opendocument.rs` | ✅
| Apple iWork | Pages/Numbers/Keynote | `PagesConverter` `NumbersConverter` `KeynoteConverter` | .pages .numbers .key | `tests/legacy_office.rs` (keynote) | ✅
| Notebooks | Jupyter | `JupyterConverter` | .ipynb | `tests/jupyter.rs` | ✅
| Presentations | PDF to slides | `PdfConverter` | .pdf | `tests/pdf.rs` | ✅
| RSS/Atom | News feeds | `RssConverter` | .xml (rss/atom) | `tests/rss.rs` | ✅
| Spreadsheets | CSV | `CsvConverter` | .csv | `tests/csv.rs` | ✅
| SQLite | SQLite DB | `SqliteConverter` | .sqlite .db | `tests/sqlite.rs` | ✅
| Archives (container) | iWork/Office within ZIP | `ArchiveConverter` | .zip | `tests/archive.rs` | ✅

Legend: ✅ = has dedicated tests; ⚪ = handled but not directly covered by a standalone test file.

## Test Totals
- Total passed: 198
- Ignored: 3 (1 PDF edge case, 2 ZIP edge cases)

## Notes
- DocBook now accepts `.docbook`, `.docbook4`, `.docbook5`, and `.dbk`.
- Markdown table test uses Pandoc simple-table fixtures, so dash-based table markup is expected (not pipe tables).
- PDF converter still has one ignored test; keep it in mind for future fixes.

## How to run the full suite
```sh
cargo test
```

## Adding new formats
1. Implement `DocumentConverter` with `supported_extensions()`.
2. Register the converter in `MarkItDown::with_store` in `src/lib.rs`.
3. Add fixtures under `tests/test_documents/<format>`.
4. Add a focused test file under `tests/<format>.rs`.
5. Keep converters returning Markdown via `Document` -> `Page` -> `ContentBlock`.
