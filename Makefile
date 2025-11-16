# Makefile for BWFS Project

.PHONY: all build release test clean install help

# Default target
all: build

# Build in debug mode
build:
	@echo "Building BWFS in debug mode..."
	cargo build

# Build in release mode
release:
	@echo "Building BWFS in release mode..."
	cargo build --release

# Run tests
test:
	@echo "Running tests..."
	cargo test

# Run clippy for linting
lint:
	@echo "Running clippy..."
	cargo clippy -- -D warnings

# Format code
fmt:
	@echo "Formatting code..."
	cargo fmt

# Check formatting
check-fmt:
	@echo "Checking code formatting..."
	cargo fmt -- --check

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf bwfs_data/
	rm -f config.local.ini

# Install binaries to system (requires sudo)
install: release
	@echo "Installing binaries to /usr/local/bin..."
	sudo cp target/release/mkfs.bwfs /usr/local/bin/
	sudo cp target/release/mount.bwfs /usr/local/bin/
	@echo "Installation complete!"

# Create a test filesystem and mount it
demo: release
	@echo "Creating demo filesystem..."
	./target/release/mkfs.bwfs -c config.ini
	@echo ""
	@echo "Creating mount point..."
	mkdir -p /tmp/bwfs_demo
	@echo ""
	@echo "Mounting filesystem..."
	@echo "Run: ./target/release/mount.bwfs -c config.ini /tmp/bwfs_demo -f"
	@echo "To unmount: fusermount -u /tmp/bwfs_demo"

# Show help
help:
	@echo "BWFS Makefile - Available targets:"
	@echo ""
	@echo "  make build       - Build project in debug mode"
	@echo "  make release     - Build project in release mode"
	@echo "  make test        - Run tests"
	@echo "  make lint        - Run clippy linter"
	@echo "  make fmt         - Format code"
	@echo "  make check-fmt   - Check code formatting"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make install     - Install binaries (requires sudo)"
	@echo "  make demo        - Create and mount demo filesystem"
	@echo "  make help        - Show this help message"
