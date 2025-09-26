#!/bin/bash

# Quick local build script for TransADIF

set -e

echo "Building TransADIF for local platform..."

# Build with release optimizations
cargo build --release

echo ""
echo "✅ Build complete!"
echo "📁 Binaries available at:"
echo "   ./target/release/transadif"
echo "   ./target/release/test_runner"

echo ""
echo "📊 Binary sizes:"
ls -lh target/release/transadif target/release/test_runner 2>/dev/null | awk '{print "   " $9 ": " $5}'

echo ""
echo "🧪 Running tests..."
./target/release/test_runner

echo ""
echo "🚀 Quick test:"
echo "   ./target/release/transadif --version"
./target/release/transadif --version 2>/dev/null || echo "   (Version display not implemented yet)"
