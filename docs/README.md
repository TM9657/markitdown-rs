# Documentation Index

Welcome to the markitdown-rs documentation! Here's a comprehensive guide to all available resources.

## 📖 Core Documentation

### [README.md](../README.md)
**Start here!** Quick overview, installation, and basic usage examples for both CLI and Rust API.

### [FORMATS.md](FORMATS.md)
Complete reference of **40+ supported formats** including:
- Detailed capabilities for each format
- Known limitations and accuracy notes
- Format categories (Documents, Spreadsheets, Ebooks, Archives, etc.)

### [ARCHITECTURE.md](ARCHITECTURE.md)
Deep dive into how markitdown-rs works:
- Core data model and architecture
- Converter pattern and how to implement
- Complete examples for simple and complex converters
- Multi-page format handling
- Performance considerations
- Full step-by-step guide to adding new formats

### [TESTING.md](TESTING.md)
Comprehensive testing guide:
- How to run tests (all, specific formats, with debugging)
- Test statistics (198+ passing tests)
- Writing tests for new formats
- Best practices and common issues
- Coverage reporting

### [FORMAT_COVERAGE.md](FORMAT_COVERAGE.md)
Quick reference matrix showing:
- All converters and their extensions
- Which test file covers each format
- Overall test statistics

### [CONTRIBUTING.md](../CONTRIBUTING.md)
Guidelines for contributing:
- Setting up development environment
- Workflow for reporting bugs and requesting features
- Step-by-step guide for adding formats
- Code style and testing requirements
- Common issues and troubleshooting

## 🚀 Quick Starts

### For Users
1. [Install](../README.md#command-line)
2. [Use CLI](../README.md#command-line) or [Rust API](../README.md#rust-api)
3. [Check FORMATS.md](FORMATS.md) for format support

### For Developers
1. [Setup](../CONTRIBUTING.md#setting-up)
2. [Understand Architecture](ARCHITECTURE.md)
3. [Look at Examples](ARCHITECTURE.md#minimal-example-text-converter)
4. [Write Tests](TESTING.md#test-file-template)
5. [Submit PR](../CONTRIBUTING.md#creating-a-branch)

### For Format Implementers
1. [Read Architecture Guide](ARCHITECTURE.md#adding-a-new-format)
2. [Follow the Checklist](../CONTRIBUTING.md#adding-a-new-format)
3. [Write Tests](TESTING.md#writing-tests-for-a-new-format)
4. [Update Documentation](../CONTRIBUTING.md#documentation-updates)

## 📊 Reference Tables

### Format Support
See [FORMAT_COVERAGE.md](FORMAT_COVERAGE.md) for complete matrix of:
- 40+ formats with converter names
- Supported file extensions
- Test file locations
- Implementation status

### Test Summary
| Category | Count |
|----------|-------|
| Total Tests | **198** ✅ |
| Ignored Tests | 3 ⚠️ |
| Format Categories | 31 |
| Formats Supported | 40+ |

See [TESTING.md](TESTING.md#test-statistics) for full breakdown.

## 🔍 Finding What You Need

**I want to...**

- **Convert a document** → [README.md - Quick Start](../README.md#quick-start)
- **Use the CLI** → [README.md - Command-Line](../README.md#command-line)
- **Use the Rust API** → [README.md - Rust API](../README.md#rust-api)
- **Know if a format is supported** → [FORMATS.md](FORMATS.md)
- **Understand how converters work** → [ARCHITECTURE.md - Core Architecture](ARCHITECTURE.md#core-architecture)
- **Implement a new format** → [ARCHITECTURE.md - Adding Formats](ARCHITECTURE.md#adding-a-new-format)
- **Write tests for a format** → [TESTING.md - Writing Tests](TESTING.md#writing-tests-for-a-new-format)
- **Debug a failing conversion** → [TESTING.md - Debugging](TESTING.md#debugging-failed-tests)
- **Contribute code** → [CONTRIBUTING.md](../CONTRIBUTING.md)
- **Report a bug** → [CONTRIBUTING.md - Reporting Bugs](../CONTRIBUTING.md#reporting-bugs)
- **Request a feature** → [CONTRIBUTING.md - Feature Requests](../CONTRIBUTING.md#requesting-features)

## 📂 File Organization

```
markitdown-rs/
├── README.md              # Main entry point
├── CONTRIBUTING.md        # Contribution guidelines
├── Cargo.toml            # Project manifest
├── src/
│   ├── lib.rs            # Main library interface
│   ├── model.rs          # Core data types
│   ├── error.rs          # Error types
│   └── <format>.rs       # One converter per format (40+ files)
├── tests/
│   ├── <format>.rs       # Tests for each format (31 test files)
│   └── test_documents/   # Test fixtures organized by format
├── docs/
│   ├── FORMATS.md              # Format reference (this file)
│   ├── ARCHITECTURE.md         # Architecture & development guide
│   ├── TESTING.md              # Testing guide
│   ├── FORMAT_COVERAGE.md      # Converter matrix
│   └── README.md               # Documentation index (this file)
└── benches/
    └── conversion.rs           # Benchmarks
```

## 🔗 External Resources

- **[Rust Book](https://doc.rust-lang.org/book/)** – Learn Rust
- **[Tokio Documentation](https://tokio.rs/)** – Async runtime
- **[Original markitdown](https://github.com/microsoft/markitdown)** – Python reference implementation
- **[Apache Tika](https://github.com/apache/tika)** – Test file sources

## ❓ Frequently Asked Questions

**Q: What's the easiest way to get started?**
A: Start with [README.md](../README.md) for your use case (CLI or API).

**Q: How do I add support for a new format?**
A: Follow the [step-by-step guide in ARCHITECTURE.md](ARCHITECTURE.md#adding-a-new-format).

**Q: Where do I find test examples?**
A: See [TESTING.md - Test File Template](TESTING.md#test-file-template).

**Q: Is the library production-ready?**
A: Yes! All 198 tests pass. See [TESTING.md](TESTING.md#test-statistics).

**Q: What if my format isn't supported?**
A: Check [FORMATS.md](FORMATS.md) first. If not listed, [request it](../CONTRIBUTING.md#requesting-features).

**Q: How do I report a bug?**
A: Follow [CONTRIBUTING.md - Reporting Bugs](../CONTRIBUTING.md#reporting-bugs).

## 📈 Project Statistics

- **40+ Formats** supported
- **198 Tests** passing
- **3 Ignored** tests (edge cases)
- **31 Categories** of formats
- **Async-first** design with Tokio
- **Production-ready** code

## 📝 Latest Updates

See [CHANGELOG.md](../CHANGELOG.md) for recent changes and releases.

---

**Need help?** Open an [issue](../../issues) or [discussion](../../discussions)!

**Want to contribute?** Start with [CONTRIBUTING.md](../CONTRIBUTING.md)!
