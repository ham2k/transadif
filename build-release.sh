#!/bin/bash
set -e

# TransADIF Release Build Script
# Builds optimized binaries for distribution

VERSION=${1:-$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')}
echo "Building TransADIF v$VERSION for distribution..."

# Create release directory
mkdir -p releases

# Build targets
TARGETS=(
    "x86_64-unknown-linux-gnu:linux-x64"
    "aarch64-unknown-linux-gnu:linux-arm64"
    "x86_64-pc-windows-gnu:windows-x64"
    "x86_64-apple-darwin:macos-x64"
    "aarch64-apple-darwin:macos-arm64"
)

# Clean previous builds
cargo clean

for target_info in "${TARGETS[@]}"; do
    IFS=':' read -r target name <<< "$target_info"

    echo "Building for $target ($name)..."

    # Build with optimizations
    RUSTFLAGS="-C target-cpu=native" cargo build \
        --release \
        --target "$target" \
        --bins

    # Determine binary extension
    if [[ "$target" == *"windows"* ]]; then
        binary_ext=".exe"
    else
        binary_ext=""
    fi

    # Create distribution directory
    dist_dir="releases/transadif-v$VERSION-$name"
    mkdir -p "$dist_dir"

    # Copy binaries
    cp "target/$target/release/transadif$binary_ext" "$dist_dir/"
    cp "target/$target/release/test-runner$binary_ext" "$dist_dir/"

    # Copy documentation
    cp README.md "$dist_dir/"
    cp LICENSE "$dist_dir/"
    cp GOALS.md "$dist_dir/"

    # Create archive
    if [[ "$target" == *"windows"* ]]; then
        # Windows - create ZIP
        (cd releases && zip -r "transadif-v$VERSION-$name.zip" "transadif-v$VERSION-$name/")
    else
        # Unix - create tar.gz
        (cd releases && tar -czf "transadif-v$VERSION-$name.tar.gz" "transadif-v$VERSION-$name/")
    fi

    # Clean up directory
    rm -rf "$dist_dir"

    echo "✓ Built $name"
done

echo ""
echo "Release binaries created in releases/ directory:"
ls -la releases/

echo ""
echo "To test a binary:"
echo "  tar -xzf releases/transadif-v$VERSION-linux-x64.tar.gz"
echo "  ./transadif-v$VERSION-linux-x64/transadif --version"