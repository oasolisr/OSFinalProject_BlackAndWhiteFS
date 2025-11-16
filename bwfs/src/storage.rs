use image::{ImageBuffer, Luma};
use std::path::PathBuf;
use std::fs;
use anyhow::Result;

/// Block storage using black and white images
/// Each pixel can store 1 bit of information (black=0, white=1)
pub struct BlockStorage {
    /// Base path for storing images
    base_path: PathBuf,
    
    /// Block dimensions (width x height in pixels)
    block_width: u32,
    block_height: u32,
    
    /// Bytes per block (width * height / 8)
    bytes_per_block: usize,
    
    /// Total number of blocks
    total_blocks: u32,
    
    /// Filesystem fingerprint
    fingerprint: String,
}

impl BlockStorage {
    /// Create a new block storage
    pub fn new(
        base_path: &str,
        block_width: u32,
        block_height: u32,
        total_blocks: u32,
        fingerprint: String,
    ) -> Result<Self> {
        let base_path = PathBuf::from(base_path);
        fs::create_dir_all(&base_path)?;
        
        let bytes_per_block = ((block_width * block_height) / 8) as usize;
        
        Ok(Self {
            base_path,
            block_width,
            block_height,
            bytes_per_block,
            total_blocks,
            fingerprint,
        })
    }
    
    /// Get the image path for a block number
    fn get_block_path(&self, block_num: u32) -> PathBuf {
        self.base_path.join(format!("block_{:08}.png", block_num))
    }
    
    /// Initialize a new block (create empty image)
    pub fn init_block(&self, block_num: u32) -> Result<()> {
        if block_num >= self.total_blocks {
            anyhow::bail!("Block number {} exceeds total blocks", block_num);
        }
        
        // Create a white image (all bits set to 1 = empty)
        let img = ImageBuffer::from_pixel(
            self.block_width,
            self.block_height,
            Luma([255u8])
        );
        
        let path = self.get_block_path(block_num);
        img.save(&path)?;
        
        Ok(())
    }
    
    /// Read data from a block
    pub fn read_block(&self, block_num: u32) -> Result<Vec<u8>> {
        if block_num >= self.total_blocks {
            anyhow::bail!("Block number {} exceeds total blocks", block_num);
        }
        
        let path = self.get_block_path(block_num);
        if !path.exists() {
            // Return empty block if doesn't exist
            return Ok(vec![0; self.bytes_per_block]);
        }
        
        let img = image::open(&path)?.to_luma8();
        
        // Convert pixels to bytes
        let mut data = Vec::with_capacity(self.bytes_per_block);
        let pixels = img.as_raw();
        
        for chunk in pixels.chunks(8) {
            let mut byte = 0u8;
            for (i, &pixel) in chunk.iter().enumerate() {
                // White (255) = 1, Black (0) = 0
                if pixel > 127 {
                    byte |= 1 << (7 - i);
                }
            }
            data.push(byte);
        }
        
        Ok(data)
    }
    
    /// Write data to a block
    pub fn write_block(&self, block_num: u32, data: &[u8]) -> Result<()> {
        if block_num >= self.total_blocks {
            anyhow::bail!("Block number {} exceeds total blocks", block_num);
        }
        
        if data.len() > self.bytes_per_block {
            anyhow::bail!("Data size exceeds block capacity");
        }
        
        // Convert bytes to pixels
        let mut pixels = Vec::with_capacity((self.block_width * self.block_height) as usize);
        
        for &byte in data {
            for i in 0..8 {
                let bit = (byte >> (7 - i)) & 1;
                // 1 = white (255), 0 = black (0)
                pixels.push(if bit == 1 { 255u8 } else { 0u8 });
            }
        }
        
        // Pad with white pixels if needed
        while pixels.len() < (self.block_width * self.block_height) as usize {
            pixels.push(255);
        }
        
        let img: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::from_vec(
            self.block_width,
            self.block_height,
            pixels
        ).ok_or_else(|| anyhow::anyhow!("Failed to create image from pixels"))?;
        
        let path = self.get_block_path(block_num);
        img.save(&path)?;
        
        Ok(())
    }
    
    /// Check if a block exists
    pub fn block_exists(&self, block_num: u32) -> bool {
        self.get_block_path(block_num).exists()
    }
    
    /// Get bytes per block
    pub fn bytes_per_block(&self) -> usize {
        self.bytes_per_block
    }
    
    /// Write fingerprint to block 0 (superblock)
    pub fn write_fingerprint(&self) -> Result<()> {
        let mut data = vec![0u8; self.bytes_per_block];
        let fingerprint_bytes = self.fingerprint.as_bytes();
        let len = fingerprint_bytes.len().min(self.bytes_per_block);
        data[..len].copy_from_slice(&fingerprint_bytes[..len]);
        
        self.write_block(0, &data)?;
        Ok(())
    }
    
    /// Read and verify fingerprint from block 0
    pub fn verify_fingerprint(&self) -> Result<bool> {
        let data = self.read_block(0)?;
        let fingerprint_bytes = self.fingerprint.as_bytes();
        
        Ok(data.starts_with(fingerprint_bytes))
    }
}

/// Bitmap for tracking free/used blocks and inodes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Bitmap {
    bits: Vec<u8>,
    size: usize,
}

impl Bitmap {
    /// Create a new bitmap with all bits set to free (1)
    pub fn new(size: usize) -> Self {
        let byte_size = (size + 7) / 8;
        Self {
            bits: vec![0x00; byte_size],
            size,
        }
    }
    
    /// Check if a bit is set (allocated)
    pub fn is_set(&self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        (self.bits[byte_idx] & (1 << bit_idx)) != 0
    }
    
    /// Set a bit (mark as allocated)
    pub fn set(&mut self, index: usize) {
        if index >= self.size {
            return;
        }
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        self.bits[byte_idx] |= 1 << bit_idx;
    }
    
    /// Clear a bit (mark as free)
    pub fn clear(&mut self, index: usize) {
        if index >= self.size {
            return;
        }
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        self.bits[byte_idx] &= !(1 << bit_idx);
    }
    
    /// Find first free bit and allocate it
    pub fn allocate(&mut self) -> Option<usize> {
        for i in 0..self.size {
            if !self.is_set(i) {
                self.set(i);
                return Some(i);
            }
        }
        None
    }
    
    /// Deallocate a bit
    pub fn deallocate(&mut self, index: usize) {
        self.clear(index);
    }
    
    /// Get raw bitmap data
    pub fn as_bytes(&self) -> &[u8] {
        &self.bits
    }
    
    /// Load bitmap from bytes
    pub fn from_bytes(data: &[u8], size: usize) -> Self {
        let mut bits = data.to_vec();
        let required_bytes = (size + 7) / 8;
        bits.resize(required_bytes, 0xFF);
        
        Self { bits, size }
    }
}
