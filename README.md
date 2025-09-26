# TransADIF

A command-line tool for dealing with transcoding and correcting text encoding issues in [ADIF](https://www.adif.org/) files.

Created by Sebastian Delmont <ki2d@ham2k.net> - [Ham2K Technologies](https://ham2k.net)

## Overview

TransADIF is a Rust-based tool designed to process [ADIF (Amateur Data Interchange Format)](https://www.adif.org/) files with intelligent encoding detection and correction. It handles various text encoding scenarios commonly encountered in amateur radio logging files, ensuring proper Unicode representation regardless of the original encoding.

## Features

- **Smart Encoding Detection**: Automatically detects ASCII, ISO-8859-1, and UTF-8 encodings
- **Field Count Interpretation**: Intelligently handles field counts specified as bytes vs. characters
- **UTF-8 Corruption Correction**: Fixes double-encoded and corrupted UTF-8 sequences
- **Entity Reference Processing**: Handles HTML/XML entity references in field data
- **Multiple Output Encodings**: Supports UTF-8, ASCII, and various code pages
- **Comprehensive ADIF Parsing**: Full support for ADIF headers, records, and fields
- **Cross-platform**: Works on Linux, Windows, and macOS

## Installation

### From Source

```bash
git clone https://github.com/ham2k/transadif.git
cd transadif
cargo build --release
```

The compiled binaries will be available in `target/release/`:
- `transadif` - Main processing tool
- `test_runner` - Test validation tool

## Usage

### Basic Usage

```bash
# Process a file (output to stdout)
transadif input.adi

# Process with output file
transadif input.adi -o output.adi

# Process from stdin
cat input.adi | transadif > output.adi
```

### Command Line Options

```
transadif <input-file> [OPTIONS]

OPTIONS:
    -h, --help                          Show help information
    -v, --version                       Show version information
    -o, --output <output-file>          Write the output to a file
    -i, --input-encoding <encoding>     Suggested encoding for input file
    -e, --encoding <encoding>           Output encoding [default: UTF-8]
    -t, --transcode                     Transcode compatible characters
    -r, --replace <character>           Replace incompatible chars [default: ?]
    -d, --delete                        Delete incompatible characters
    -a, --ascii                         Transliterate to ASCII without diacritics
    -s, --strict                        Strict mode - no automatic corrections
```

### Supported Encodings

**Input Detection:**
- UTF-8
- ISO-8859-1 (Latin-1)
- Windows-1252
- ASCII

**Output Formats:**
- UTF-8 (default)
- ASCII
- ISO-8859-1
- Windows-1252

### Examples

```bash
# Convert to UTF-8 with automatic encoding detection
transadif logbook.adi -o logbook_utf8.adi

# Force input encoding and convert to ASCII
transadif -i ISO-8859-1 -e ASCII logbook.adi -o logbook_ascii.adi

# Transliterate Unicode to ASCII (removes diacritics)
transadif -a -e ASCII international.adi -o ascii_clean.adi

# Strict mode (no automatic corrections)
transadif -s problematic.adi -o strict_output.adi

# Replace incompatible characters with underscore
transadif -e ASCII -r _ mixed_encoding.adi -o clean.adi
```

## How It Works

### Encoding Detection

TransADIF uses intelligent heuristics to detect the input file encoding:

1. **Header Field Check**: Looks for explicit `<encoding:N>` fields
2. **UTF-8 Validation**: Tests for valid UTF-8 sequences
3. **High Byte Analysis**: Analyzes byte patterns for encoding signatures
4. **Fallback Logic**: Uses ISO-8859-1 for ambiguous cases

### Field Count Interpretation

ADIF field counts can be ambiguous (bytes vs. characters). TransADIF:

- Detects when field counts don't match the expected data
- Reinterprets counts based on encoding and content analysis
- Handles both undercount and overcount scenarios
- Preserves data integrity during reinterpretation

### Data Correction

The tool automatically corrects common encoding issues:

- **Double-encoded UTF-8**: Fixes UTF-8 → ISO-8859-1 → UTF-8 corruption
- **Entity References**: Converts `&amp;`, `&lt;`, `&#123;` etc. to Unicode
- **Mixed Encodings**: Handles files with inconsistent encoding
- **Byte Sequence Errors**: Repairs corrupted multi-byte sequences

## ADIF Format Support

TransADIF fully supports the ADIF specification:

- **Headers**: Preamble text and header fields
- **Records**: QSO records with proper field parsing
- **Fields**: All field types with length and type specifiers
- **Tags**: Proper handling of `<eoh>` and `<eor>` markers
- **Excess Data**: Preserves whitespace and formatting

## Test Suite

The project includes a comprehensive test suite covering various ADIF text encoding scenarios.

### Running Tests

```bash
# Build the test runner first
cargo build --bin test_runner

# Run all tests
./target/debug/test_runner

# Or build and run in one command
cargo run --bin test_runner
```

### Test Filtering

```bash
# Run specific test categories
./target/debug/test_runner -f plain
./target/debug/test_runner -f undercount
./target/debug/test_runner -f sneaky

# Run tests from specific directory
./target/debug/test_runner -d test-cases/01-plain-examples
./target/debug/test_runner -d test-cases/02-field-length
```

## Development

### Project Structure

```
src/
├── main.rs              # CLI application entry point
├── lib.rs               # Library exports
├── adif.rs              # ADIF parsing logic
├── encoding.rs          # Encoding detection and correction
├── error.rs             # Error types and handling
├── output.rs            # ADIF output generation
└── bin/
    └── test_runner.rs   # Test execution framework

test-cases/
├── 01-plain-examples/   # Basic encoding tests
└── 02-field-length/     # Field count interpretation tests
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- input.adi
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo run --bin test_runner`
5. Submit a pull request

## License

This project is licensed under the MIT License. See the LICENSE file for details.

## Author

**Sebastian Delmont KI2D** - [Ham2K Technologies](https://ham2k.net)

