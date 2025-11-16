# BWFS Build Script for Windows
# Run this in PowerShell

Write-Host "======================================" -ForegroundColor Cyan
Write-Host "   BWFS Build Script for Windows" -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""

# Check if Rust is installed
Write-Host "Checking Rust installation..." -ForegroundColor Yellow
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Rust is not installed" -ForegroundColor Red
    Write-Host "Please install Rust from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

Write-Host "Rust version:" -ForegroundColor Green
rustc --version
cargo --version
Write-Host ""

# Note about FUSE
Write-Host "NOTE: FUSE is not available on Windows" -ForegroundColor Yellow
Write-Host "This project requires Linux to run the filesystem" -ForegroundColor Yellow
Write-Host "You can:" -ForegroundColor Yellow
Write-Host "  1. Use WSL2 (Windows Subsystem for Linux)" -ForegroundColor Cyan
Write-Host "  2. Use a Linux VM" -ForegroundColor Cyan
Write-Host "  3. Compile for cross-compilation to Linux" -ForegroundColor Cyan
Write-Host ""

# Parse arguments
$mode = "release"
if ($args -contains "--debug") {
    $mode = "debug"
}

# Build the project
if ($mode -eq "release") {
    Write-Host "Building BWFS in RELEASE mode..." -ForegroundColor Green
    cargo build --release
    $binaryPath = "target\release"
} else {
    Write-Host "Building BWFS in DEBUG mode..." -ForegroundColor Green
    cargo build
    $binaryPath = "target\debug"
}

Write-Host ""
Write-Host "======================================" -ForegroundColor Cyan
Write-Host "   Build Complete!" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Binaries location:" -ForegroundColor Yellow
Write-Host "  mkfs.bwfs:  $binaryPath\mkfs.bwfs.exe" -ForegroundColor Cyan
Write-Host "  mount.bwfs: $binaryPath\mount.bwfs.exe" -ForegroundColor Cyan
Write-Host ""
Write-Host "To run on Linux (WSL2):" -ForegroundColor Yellow
Write-Host "  1. wsl" -ForegroundColor Cyan
Write-Host "  2. cd /mnt/c/Users/..." -ForegroundColor Cyan
Write-Host "  3. ./target/release/mkfs.bwfs -c config.ini" -ForegroundColor Cyan
Write-Host "  4. ./target/release/mount.bwfs -c config.ini /tmp/bwfs" -ForegroundColor Cyan
Write-Host ""
