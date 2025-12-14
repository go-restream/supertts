# Makefile for superTTS
.PHONY: build build-server build-docker build-cli download run run-docker clean deps test fmt check help

build: build-server

build-server:
	cargo build --release

build-cli:
	cargo build --release

build-docker:
	sh ./build.sh

download:
	git clone https://hf-mirror.com/Supertone/supertonic assets

run:
	target/release/supertts --openai --config config.json

run-docker:
	docker-compose up

clean:
	cargo clean

deps:
	cargo fetch

test:
	cargo test

fmt:
	cargo fmt

check:
	cargo check

# Show help
help:
	@echo "Available targets:"
	@echo "  build        - Build the server version (default)"
	@echo "  build-docker - Build docker image"
	@echo "  build-server - Build server version with --features server --release"
	@echo "  build-cli    - Build CLI version (debug build)"
	@echo "  download     - Download model assets from hf-mirror.com"
	@echo "  run          - Run server"
	@echo "  run-docker   - Run server in docker"
	@echo "  clean        - Clean build artifacts"
	@echo "  deps         - Install dependencies"
	@echo "  test         - Run tests"
	@echo "  fmt          - Format code"
	@echo "  check        - Check code without building"
	@echo "  help         - Show this help message"