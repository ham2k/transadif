#!/bin/bash
set -e

# Simple TransADIF Release Build Script
# Builds for platforms that can be cross-compiled on this system

VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
echo "Building TransADIF v$VERSION for distribution..."

# Create release directory
mkdir -p releases

# Clean previous builds
cargo clean

echo "Building optimized release binary for current platform..."

# Build with full optimizations
RUSTFLAGS="-C target-cpu=native -C strip=symbols" cargo build --release

# Determine current platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ $(uname -m) == "arm64" ]]; then
        PLATFORM="macos-arm64"
    else
        PLATFORM="macos-x64"
    fi
elif [[ "$OSTYPE" == "linux"* ]]; then
    if [[ $(uname -m) == "aarch64" ]]; then
        PLATFORM="linux-arm64"
    else
        PLATFORM="linux-x64"
    fi
else
    PLATFORM="unknown"
fi

# Create distribution directory
DIST_DIR="releases/transadif-v$VERSION-$PLATFORM"
mkdir -p "$DIST_DIR"

# Copy binaries
cp target/release/transadif "$DIST_DIR/"
cp target/release/test-runner "$DIST_DIR/"

# Copy documentation
cp README.md "$DIST_DIR/"
cp LICENSE "$DIST_DIR/"
cp GOALS.md "$DIST_DIR/"

# Copy test cases
cp -r test-cases "$DIST_DIR/"

# Create archive
(cd releases && tar -czf "transadif-v$VERSION-$PLATFORM.tar.gz" "transadif-v$VERSION-$PLATFORM/")

# Create checksums
(cd releases && shasum -a 256 "transadif-v$VERSION-$PLATFORM.tar.gz" > "transadif-v$VERSION-$PLATFORM.tar.gz.sha256")

# Clean up directory
rm -rf "$DIST_DIR"

echo "✓ Built for $PLATFORM"
echo ""
echo "Release archive created:"
ls -la "releases/transadif-v$VERSION-$PLATFORM.tar.gz"

echo ""
echo "To test the binary:"
echo "  tar -xzf releases/transadif-v$VERSION-$PLATFORM.tar.gz"
echo "  ./transadif-v$VERSION-$PLATFORM/transadif --version"
echo "  ./transadif-v$VERSION-$PLATFORM/test-runner"

echo ""
echo "Binary info:"
file "target/release/transadif"
echo "Size: $(du -h target/release/transadif | cut -f1)"