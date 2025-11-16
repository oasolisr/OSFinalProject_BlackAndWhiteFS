#!/bin/bash
# Build script for BWFS

set -e

echo "======================================"
echo "   BWFS Build Script"
echo "======================================"
echo

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust is not installed"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "✓ Rust version:"
rustc --version
cargo --version
echo

# Check if FUSE is available
if ! pkg-config --exists fuse; then
    echo "⚠️  Warning: FUSE development files not found"
    echo "On Ubuntu/Debian: sudo apt-get install fuse libfuse-dev"
    echo "On Fedora: sudo dnf install fuse fuse-devel"
    echo
fi

# Parse arguments
MODE="release"
VERBOSE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --debug)
            MODE="debug"
            shift
            ;;
        --verbose)
            VERBOSE="--verbose"
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo
            echo "Options:"
            echo "  --debug      Build in debug mode (default: release)"
            echo "  --verbose    Show verbose output"
            echo "  --help       Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Build the project
if [ "$MODE" = "release" ]; then
    echo "🔨 Building BWFS in RELEASE mode..."
    cargo build --release $VERBOSE
    BINARY_PATH="target/release"
else
    echo "🔨 Building BWFS in DEBUG mode..."
    cargo build $VERBOSE
    BINARY_PATH="target/debug"
fi

echo
echo "======================================"
echo "   Build Complete!"
echo "======================================"
echo
echo "Binaries location:"
echo "  mkfs.bwfs:  $BINARY_PATH/mkfs.bwfs"
echo "  mount.bwfs: $BINARY_PATH/mount.bwfs"
echo
echo "To create a filesystem:"
echo "  ./$BINARY_PATH/mkfs.bwfs -c config.ini"
echo
echo "To mount the filesystem:"
echo "  ./$BINARY_PATH/mount.bwfs -c config.ini /path/to/mountpoint"
echo
echo "To install system-wide (requires sudo):"
echo "  make install"
echo
