# BWFS Project - Quick Start Guide

## What is BWFS?

BWFS (Black and White FileSystem) is a unique filesystem that stores data in black and white PNG images. Each pixel represents a bit (white=1, black=0), creating a visual representation of your data.

## Quick Start (Linux)

### 1. Install Dependencies

```bash
# Ubuntu/Debian
sudo apt-get install fuse libfuse-dev pkg-config build-essential

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Build the Project

```bash
# Simple build
cargo build --release

# Or use the build script
chmod +x build.sh
./build.sh
```

### 3. Create a Filesystem

```bash
# Use the provided config
./target/release/mkfs.bwfs -c config.ini
```

### 4. Mount and Use

```bash
# Create mount point
mkdir -p /tmp/bwfs

# Mount filesystem
./target/release/mount.bwfs -c config.ini /tmp/bwfs

# Use it!
echo "Hello BWFS!" > /tmp/bwfs/test.txt
cat /tmp/bwfs/test.txt
ls -la /tmp/bwfs

# Unmount
fusermount -u /tmp/bwfs
```

## Quick Start (WSL2 on Windows)

### 1. Install WSL2

```powershell
# In PowerShell as Administrator
wsl --install
wsl --set-default-version 2
```

### 2. Inside WSL2

```bash
# Install dependencies
sudo apt-get update
sudo apt-get install fuse libfuse-dev pkg-config build-essential

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Navigate to project (assuming it's in Windows)
cd /mnt/c/Users/YourName/Documents/TEC/SOA/Filesystem

# Build and run
cargo build --release
./target/release/mkfs.bwfs -c config.ini
mkdir -p /tmp/bwfs
./target/release/mount.bwfs -c config.ini /tmp/bwfs
```

## Project Structure

```
Filesystem/
├── bwfs/              # Core library (filesystem logic)
├── mkfs-bwfs/         # Filesystem creation tool
├── mount-bwfs/        # Mount tool
├── config.ini         # Configuration file
├── README.md          # Full documentation
└── DOCUMENTATION.md   # Project documentation (Spanish)
```

## Configuration

Edit `config.ini` to customize:

```ini
[filesystem]
name = MyBWFS                # Your filesystem name
block_width = 1000           # Block width (max 1000)
block_height = 1000          # Block height (max 1000)
total_blocks = 100           # Number of blocks
total_inodes = 1024          # Number of inodes
storage_path = ./bwfs_data   # Where to store images
fingerprint = BWFS_v1.0      # Filesystem ID
tcp_port = 9000              # Network port
```

## Capacity Calculation

- Block size: 1000×1000 pixels = 125,000 bytes (≈122 KB)
- 100 blocks = ≈12.2 MB
- 1000 blocks = ≈122 MB

## Troubleshooting

### "FUSE not found"
```bash
sudo apt-get install fuse libfuse-dev
```

### "Permission denied" when mounting
```bash
sudo usermod -a -G fuse $USER
# Then logout and login again
```

### "Device or resource busy"
```bash
fusermount -uz /tmp/bwfs
```

### Enable debug logs
```bash
RUST_LOG=debug ./target/release/mount.bwfs -c config.ini -f /tmp/bwfs
```

## Features

✓ Standard POSIX operations (read, write, create, delete, etc.)
✓ Directory support
✓ Persistent storage in PNG images
✓ Visual data representation
✓ Network support (TCP/IP)
✓ Configurable capacity

## Learn More

- Full documentation: See `README.md`
- Spanish documentation: See `DOCUMENTATION.md`
- Code documentation: Run `cargo doc --open`

## Need Help?

1. Check logs: `RUST_LOG=debug`
2. Read full README.md
3. Check GitHub issues
4. Contact: kmoragas@ic-itcr.ac.cr

## Example Session

```bash
# Build
cargo build --release

# Create filesystem (12.2 MB)
./target/release/mkfs.bwfs -c config.ini

# Mount
mkdir -p /tmp/bwfs
./target/release/mount.bwfs -c config.ini /tmp/bwfs

# Use it
echo "Testing BWFS" > /tmp/bwfs/test.txt
mkdir /tmp/bwfs/mydir
cp /etc/hosts /tmp/bwfs/mydir/
ls -la /tmp/bwfs

# Check the images!
ls -lh ./bwfs_data/
# You'll see block_00000000.png, block_00000001.png, etc.

# View a block image
display ./bwfs_data/block_00000001.png

# Unmount
fusermount -u /tmp/bwfs
```

Enjoy exploring BWFS! 🎨
