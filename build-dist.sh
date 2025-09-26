#!/bin/bash

# TransADIF Distribution Build Script
# Builds optimized binaries for multiple platforms

set -e

VERSION=$(grep '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
echo "Building TransADIF v$VERSION for distribution..."

# Clean previous builds
echo "Cleaning previous builds..."
cargo clean

# Create distribution directory
mkdir -p dist

# Function to build for a target
build_target() {
    local target=$1
    local os_name=$2
    local arch_name=$3

    echo "Building for $target ($os_name-$arch_name)..."

    # Install target if not present
    rustup target add $target 2>/dev/null || true

    # Build with release profile
    cargo build --release --target $target

    # Create distribution directory
    local dist_dir="dist/transadif-v$VERSION-$os_name-$arch_name"
    mkdir -p "$dist_dir"

    # Copy binaries
    if [[ "$target" == *"windows"* ]]; then
        cp "target/$target/release/transadif.exe" "$dist_dir/"
        cp "target/$target/release/test_runner.exe" "$dist_dir/" 2>/dev/null || true
    else
        cp "target/$target/release/transadif" "$dist_dir/"
        cp "target/$target/release/test_runner" "$dist_dir/" 2>/dev/null || true
    fi

    # Copy documentation
    cp README.md "$dist_dir/"
    cp LICENSE "$dist_dir/" 2>/dev/null || echo "MIT License - See repository for details" > "$dist_dir/LICENSE"

    # Create archive
    cd dist
    if [[ "$target" == *"windows"* ]]; then
        zip -r "transadif-v$VERSION-$os_name-$arch_name.zip" "transadif-v$VERSION-$os_name-$arch_name"
    else
        tar -czf "transadif-v$VERSION-$os_name-$arch_name.tar.gz" "transadif-v$VERSION-$os_name-$arch_name"
    fi
    cd ..

    echo "✓ Built $os_name-$arch_name"
}

# Build for common targets
echo "Building for multiple platforms..."

# macOS (current platform)
if [[ "$OSTYPE" == "darwin"* ]]; then
    build_target "x86_64-apple-darwin" "macos" "x86_64"
    build_target "aarch64-apple-darwin" "macos" "aarch64"
fi

# Linux
build_target "x86_64-unknown-linux-gnu" "linux" "x86_64"
build_target "aarch64-unknown-linux-gnu" "linux" "aarch64"

# Windows (requires cross-compilation setup)
if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    build_target "x86_64-pc-windows-gnu" "windows" "x86_64"
else
    echo "⚠️  Windows cross-compilation not available (missing mingw-w64)"
    echo "   Install with: brew install mingw-w64 (macOS) or apt install gcc-mingw-w64 (Linux)"
fi

echo ""
echo "✅ Distribution build complete!"
echo "📦 Archives created in dist/ directory:"
ls -la dist/*.{tar.gz,zip} 2>/dev/null || true

echo ""
echo "📋 Build Summary:"
echo "   Version: $VERSION"
echo "   Targets built: $(ls dist/*.{tar.gz,zip} 2>/dev/null | wc -l | tr -d ' ')"
echo "   Total size: $(du -sh dist/ | cut -f1)"

echo ""
echo "🚀 To test a build:"
echo "   tar -xzf dist/transadif-v$VERSION-linux-x86_64.tar.gz"
echo "   ./transadif-v$VERSION-linux-x86_64/transadif --version"
