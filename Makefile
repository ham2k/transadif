# TransADIF Makefile

.PHONY: all build test clean install dist help

# Default target
all: build

# Build for local development
build:
	@echo "🔨 Building TransADIF..."
	cargo build --release
	@echo "✅ Build complete!"

# Build with size optimization
build-small:
	@echo "🔨 Building TransADIF (optimized for size)..."
	cargo build --profile dist
	@echo "✅ Small build complete!"

# Run tests
test:
	@echo "🧪 Running Cargo tests..."
	cargo test
	@echo "🧪 Running integration tests..."
	./target/release/test_runner

# Build and test
check: build test

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf dist/

# Install locally (requires cargo install path in PATH)
install: build
	@echo "📦 Installing TransADIF locally..."
	cargo install --path . --force

# Build distribution packages
dist:
	@echo "📦 Building distribution packages..."
	./build-dist.sh

# Quick local build and test
dev:
	@echo "🚀 Development build..."
	./build-local.sh

# Show binary information
info: build
	@echo "📊 Binary Information:"
	@echo "   TransADIF: $(shell ls -lh target/release/transadif | awk '{print $$5}')"
	@echo "   Test Runner: $(shell ls -lh target/release/test_runner | awk '{print $$5}')"
	@echo ""
	@echo "🔍 Dependencies:"
	@cargo tree --depth 1

# Show help
help:
	@echo "TransADIF Build System"
	@echo ""
	@echo "Available targets:"
	@echo "  build       Build optimized release binaries"
	@echo "  build-small Build size-optimized binaries"
	@echo "  test        Run all tests"
	@echo "  check       Build and test"
	@echo "  clean       Clean build artifacts"
	@echo "  install     Install locally"
	@echo "  dist        Build distribution packages"
	@echo "  dev         Quick development build"
	@echo "  info        Show binary information"
	@echo "  help        Show this help message"
