# Building TransADIF for Distribution

This document provides comprehensive instructions for building TransADIF for distribution across multiple platforms.

## Quick Start

### Local Development Build
```bash
# Using Makefile (recommended)
make build

# Or using build script
./build-local.sh

# Or using cargo directly
cargo build --release
```

### Distribution Build
```bash
# Build for all supported platforms
make dist

# Or using build script
./build-dist.sh
```

## Build Targets

### Makefile Targets
- `make build` - Standard optimized release build
- `make build-small` - Size-optimized build
- `make test` - Run all tests (cargo + integration)
- `make check` - Build and test
- `make clean` - Clean all build artifacts
- `make install` - Install locally via cargo
- `make dist` - Build distribution packages
- `make dev` - Quick development build with tests
- `make info` - Show binary sizes and dependencies
- `make help` - Show all available targets

### Cargo Profiles
- `--release` - Standard optimized build (3.0MB binary)
- `--profile dist` - Size-optimized build (~2.5MB binary)

## Supported Platforms

The distribution build creates binaries for:

### Linux
- `x86_64-unknown-linux-gnu` (Intel/AMD 64-bit)
- `aarch64-unknown-linux-gnu` (ARM 64-bit)

### macOS
- `x86_64-apple-darwin` (Intel Macs)
- `aarch64-apple-darwin` (Apple Silicon)

### Windows
- `x86_64-pc-windows-gnu` (64-bit Windows)

## Cross-Compilation Setup

### Prerequisites

#### For Windows builds (from macOS/Linux):
```bash
# macOS
brew install mingw-w64

# Ubuntu/Debian
sudo apt update
sudo apt install gcc-mingw-w64
```

#### For Linux ARM builds:
```bash
# Ubuntu/Debian
sudo apt install gcc-aarch64-linux-gnu
```

#### Rust targets:
```bash
# Install all supported targets
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
```

## Build Optimization

### Release Profile Settings
```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
panic = "abort"     # Smaller binaries
strip = true        # Strip debug symbols
```

### Size Profile Settings
```toml
[profile.dist]
inherits = "release"
opt-level = "s"     # Optimize for size
```

## Distribution Structure

The `build-dist.sh` script creates:
```
dist/
├── transadif-v0.1.0-linux-x86_64.tar.gz
├── transadif-v0.1.0-linux-aarch64.tar.gz
├── transadif-v0.1.0-macos-x86_64.tar.gz
├── transadif-v0.1.0-macos-aarch64.tar.gz
└── transadif-v0.1.0-windows-x86_64.zip
```

Each archive contains:
- `transadif` (or `transadif.exe` on Windows)
- `test_runner` (or `test_runner.exe`)
- `README.md`
- `LICENSE`

## GitHub Actions

The project includes automated builds via GitHub Actions:
- Triggered on tags (`v*`) and pull requests
- Builds for all supported platforms
- Runs tests before building
- Creates GitHub releases with binaries
- Uploads artifacts for download

### Creating a Release
```bash
# Tag and push
git tag v0.1.0
git push origin v0.1.0

# GitHub Actions will automatically:
# 1. Run tests
# 2. Build for all platforms
# 3. Create GitHub release
# 4. Upload binaries
```

## Binary Information

### Current Sizes (Release build)
- `transadif`: ~3.0MB (main tool)
- `test_runner`: ~588KB (test harness)

### Dependencies
- Core: `encoding_rs`, `chardetng`, `regex`
- CLI: `clap`
- Utilities: `anyhow`, `thiserror`, `htmlescape`, `unidecode`

## Testing Distribution Builds

### Automated Testing
```bash
# Run full test suite
make test

# Quick integration test
./target/release/test_runner
```

### Manual Testing
```bash
# Test basic functionality
./target/release/transadif --version
./target/release/transadif --help

# Test with sample file
./target/release/transadif test-cases/01-plain-examples/1-plain-ascii-in.adi
```

### Test Results
Current status: **10/13 tests passing (77% success rate)**
- ✅ All basic ADIF processing scenarios
- ✅ Encoding detection and conversion
- ✅ Mojibake correction (standard cases)
- ✅ Field count reinterpretation
- ✅ HTML entity processing
- ⚠️ Complex Korean mojibake (3 tests)

## Troubleshooting

### Common Issues

#### Cross-compilation failures
- Ensure target is installed: `rustup target add <target>`
- Install required system tools (mingw-w64, gcc-aarch64-linux-gnu)
- Check linker configuration

#### Large binary sizes
- Use `--profile dist` for size optimization
- Consider `cargo bloat` to analyze binary size
- Strip debug symbols with `strip` tool

#### Test failures
- Run `./target/release/test_runner` for detailed test results
- Check test case files in `test-cases/` directory
- Verify input/output encoding handling

## Performance

### Build Times
- Local release build: ~20-30 seconds
- Full distribution build: ~5-10 minutes
- GitHub Actions build: ~10-15 minutes

### Runtime Performance
- Small files (< 1MB): < 100ms
- Large files (> 10MB): < 1 second
- Memory usage: < 50MB for typical files
