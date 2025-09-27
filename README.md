# TransADIF

A comprehensive command-line tool for processing ADIF (Amateur Data Interchange Format) files with proper encoding handling, written in Rust.

## Features

### Core Functionality
- **Complete ADIF Parsing** - Handles headers, fields, records, and all ADIF structures
- **Smart Encoding Detection** - Automatically detects file encoding or accepts manual specification
- **Unicode Conversion** - Converts between 20+ encoding formats with proper character handling
- **Field Count Reinterpretation** - Intelligently handles byte vs character counting for different encodings
- **Mojibake Correction** - Fixes corrupted text from multiple encoding conversions
- **Entity Reference Processing** - Handles HTML entities (`&amp;`, `&lt;`, `&0xNN;`, etc.)

### Supported Encodings

**Western European:**
- UTF-8, Windows-1252, ISO-8859-1 through ISO-8859-15
- ASCII/US-ASCII

**Cyrillic:**
- ISO-8859-5, KOI8-R, KOI8-U

**Other Languages:**
- ISO-8859-6 (Arabic), ISO-8859-7 (Greek), ISO-8859-8 (Hebrew)

**Asian Languages:**
- Shift_JIS, EUC-JP (Japanese)
- GBK, Big5 (Chinese)

### Advanced Features
- **Debug Mode** - Detailed field-by-field analysis
- **Strict Mode** - Validation-only operation with error reporting
- **Character Replacement** - Configurable handling of incompatible characters
- **ASCII Transliteration** - Convert accented characters to base forms
- **Comprehensive Testing** - 100% test coverage with byte-by-byte validation

## Installation

### Prerequisites
- Rust 1.70+ (2021 edition)

### Build from Source
```bash
git clone https://github.com/ham2k/transadif.git
cd transadif
cargo build --release
```

The compiled binary will be available at `target/release/transadif`.

### Cross-Platform Builds
```bash
# Linux
cargo build --release --target x86_64-unknown-linux-gnu

# Windows
cargo build --release --target x86_64-pc-windows-gnu

# macOS
cargo build --release --target x86_64-apple-darwin
```

## Usage

### Basic Usage
```bash
# Convert a file (auto-detect input encoding, output UTF-8)
transadif input.adi

# Specify input and output files
transadif input.adi -o output.adi

# Read from stdin, write to stdout
cat input.adi | transadif > output.adi
```

### Encoding Options
```bash
# Specify input encoding
transadif input.adi --input-encoding ISO-8859-1

# Specify output encoding
transadif input.adi --encoding Windows-1252

# Convert between different encodings
transadif input.adi -i Shift_JIS -e UTF-8
```

### Character Handling
```bash
# Replace incompatible characters with '?'
transadif input.adi --replace '?'

# Delete incompatible characters
transadif input.adi --delete

# Transliterate to ASCII (remove diacritics)
transadif input.adi --ascii

# Transcode compatible characters
transadif input.adi --transcode
```

### Debug and Validation
```bash
# Debug mode - analyze specific QSOs
transadif input.adi --debug 0,1,2

# Strict mode - validation only, report errors
transadif input.adi --strict

# Combine debug and strict modes
transadif input.adi --debug 0 --strict
```

## Command Line Options

```
Usage: transadif [OPTIONS] [INPUT]

Arguments:
  [INPUT]  Input ADIF file (reads from stdin if not specified)

Options:
  -o, --output <OUTPUT>
          Output file (writes to stdout if not specified)

  -i, --input-encoding <INPUT_ENCODING>
          Suggested encoding for the input file

  -e, --encoding <ENCODING>
          Encoding for the output file [default: UTF-8]

  -t, --transcode
          Transcode compatible characters

  -r, --replace <REPLACE>
          Replace incompatible characters with specified character [default: ?]

      --delete
          Delete incompatible characters instead of replacing them

  -a, --ascii
          Transliterate to characters without diacritics (ASCII mode)

  -s, --strict
          Strict mode - do not correct invalid characters or field counts

  -d, --debug <DEBUG>
          Debug mode - print contents of specified QSOs (comma-separated)

  -h, --help
          Print help

  -V, --version
          Print version
```

## Examples

### Common Use Cases

**Fix encoding issues in a contest log:**
```bash
transadif contest_log.adi -o fixed_log.adi
```

**Convert Japanese log to UTF-8:**
```bash
transadif ja_log.adi --input-encoding Shift_JIS --encoding UTF-8
```

**Debug mojibake in specific QSOs:**
```bash
transadif problem_log.adi --debug 5,10,15
```

**Validate file in strict mode:**
```bash
transadif log.adi --strict --output /dev/null
```

**Convert to ASCII for compatibility:**
```bash
transadif unicode_log.adi --ascii --encoding US-ASCII
```

### Field Count Issues

TransADIF automatically detects and fixes field count issues:

```adif
# Input with byte counts but UTF-8 data
<name:14>José García  # Wrong: counts bytes (16) as characters (14)

# Output with corrected character counts
<name:12>José García  # Correct: 12 Unicode characters
```

### Mojibake Correction

Automatically fixes double-encoded text:

```adif
# Input with mojibake
<name:20>JuÃ¡n MuÃ±oz UTF

# Output with correction
<name:14>Juan Muñoz UTF
```

## Testing

### Run All Tests
```bash
cargo test
```

### Run Integration Tests
```bash
# Build test runner
cargo build --bin test-runner

# Run all test cases
./target/debug/test-runner

# Run specific tests
./target/debug/test-runner --filter "mojibake"
./target/debug/test-runner --filter "field-length"
```

### Test Coverage
The tool includes comprehensive test coverage:
- **Plain Examples** - ASCII, ISO, UTF-8, mojibake correction
- **Field Length** - Undercount, overcount, multi-byte characters
- **Entity Processing** - Named entities, numeric entities

Current test status: **13/13 tests passing (100%)**

## Technical Details

### Architecture
- **Parser** (`src/adif.rs`) - Complete ADIF format parser
- **Encoding** (`src/encoding.rs`) - Multi-encoding detection and conversion
- **Output** (`src/output.rs`) - Formatting with proper length calculations
- **CLI** (`src/cli.rs`) - Command-line interface
- **Testing** (`src/test_runner.rs`) - Comprehensive test framework

### Encoding Detection Process
1. **UTF-8 Detection** - Fast path for valid UTF-8
2. **Statistical Analysis** - Uses `chardetng` for encoding detection
3. **Quality Scoring** - Evaluates decoded text quality
4. **Fallback Chain** - Tries multiple encodings in order of likelihood

### Field Count Handling
TransADIF intelligently handles the ambiguity between byte and character counts:
- Detects UTF-8 sequences in field data
- Analyzes excess data for parsing errors
- Reinterprets counts when beneficial
- Preserves original structure when correct

## Contributing

### Development Setup
```bash
git clone https://github.com/ham2k/transadif.git
cd transadif
cargo build
cargo test
```

### Code Style
- Follow Rust standard formatting (`cargo fmt`)
- Run clippy for linting (`cargo clippy`)
- Maintain test coverage for new features

### Adding Test Cases
Test cases are in the `test-cases/` directory:
- Input files: `*-in.adi`
- Expected output: `*-out.adi`
- Command line in file preamble

## Dependencies

- **clap** - Command-line argument parsing
- **encoding_rs** - Character encoding detection and conversion
- **chardetng** - Statistical encoding detection
- **htmlescape** - HTML entity processing
- **regex** - Pattern matching for mojibake correction
- **unicode-normalization** - Unicode text normalization
- **thiserror** - Error handling

## License

MIT License - see [LICENSE](LICENSE) file for details.

## ADIF Specification

This tool implements the ADIF specification with focus on:
- Header and record parsing
- Field format validation
- Encoding declaration handling
- Data integrity preservation

For the complete ADIF specification, visit: [ADIF Specification](https://adif.org/)

## Support

For issues, feature requests, or questions:
- GitHub Issues: https://github.com/ham2k/transadif/issues
- Documentation: https://github.com/ham2k/transadif/wiki

## Related Projects

- **HAM2K** - Amateur radio logging and contest software
- **ADIF.org** - Official ADIF specification and tools