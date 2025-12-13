# Makefile for superTTS

.PHONY: build build-server build-cli download clean deps test fmt check help

# Default target
build: build-server

# Build server version with release optimization
build-server:
	cargo build --release

# Build CLI version (debug build)
build-cli:
	cargo build --release

# Download model assets
download:
	git clone https://hf-mirror.com/Supertone/supertonic assets

run:
	target/release/supertts --openai --config config.json
# Clean build artifacts
clean:
	cargo clean

# Install dependencies
deps:
	cargo fetch

# Run tests
test:
	cargo test

# Format code
fmt:
	cargo fmt

# Check code without building
check:
	cargo check

# Show help
help:
	@echo "Available targets:"
	@echo "  build        - Build the server version (default)"
	@echo "  build-server - Build server version with --features server --release"
	@echo "  build-cli    - Build CLI version (debug build)"
	@echo "  download     - Download model assets from hf-mirror.com"
	@echo "  clean        - Clean build artifacts"
	@echo "  deps         - Install dependencies"
	@echo "  test         - Run tests"
	@echo "  fmt          - Format code"
	@echo "  check        - Check code without building"
	@echo "  help         - Show this help message"