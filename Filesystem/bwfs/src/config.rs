use serde::{Deserialize, Serialize};

/// Configuration for BWFS filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Filesystem name
    pub name: String,
    
    /// Block size in pixels (max 1000x1000)
    pub block_width: u32,
    pub block_height: u32,
    
    /// Total number of blocks
    pub total_blocks: u32,
    
    /// Number of inodes
    pub total_inodes: u32,
    
    /// Path to store filesystem images
    pub storage_path: String,
    
    /// Fingerprint for filesystem identification
    pub fingerprint: String,
    
    /// Distributed nodes (optional)
    pub distributed_nodes: Vec<String>,
    
    /// TCP port for network communication
    pub tcp_port: u16,
}

impl Config {
    /// Load configuration from INI file
    pub fn from_ini(path: &str) -> anyhow::Result<Self> {
        use configparser::ini::Ini;
        let mut ini = Ini::new();
        ini.load(path).map_err(|e| anyhow::anyhow!("Failed to load INI: {}", e))?;
        
        let name = ini.get("filesystem", "name")
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' field"))?;
        
        let block_width = ini.get("filesystem", "block_width")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);
        
        let block_height = ini.get("filesystem", "block_height")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);
        
        let total_blocks = ini.get("filesystem", "total_blocks")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing 'total_blocks' field"))?;
        
        let total_inodes = ini.get("filesystem", "total_inodes")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1024);
        
        let storage_path = ini.get("filesystem", "storage_path")
            .ok_or_else(|| anyhow::anyhow!("Missing 'storage_path' field"))?;
        
        let fingerprint = ini.get("filesystem", "fingerprint")
            .unwrap_or_else(|| "BWFS".to_string());
        
        let tcp_port = ini.get("filesystem", "tcp_port")
            .and_then(|s| s.parse().ok())
            .unwrap_or(9000);
        
        // Parse distributed nodes if present
        let mut distributed_nodes = Vec::new();
        for i in 1..10 {
            if let Some(node) = ini.get("network", &format!("node{}", i)) {
                distributed_nodes.push(node);
            }
        }
        
        Ok(Config {
            name,
            block_width,
            block_height,
            total_blocks,
            total_inodes,
            storage_path,
            fingerprint,
            distributed_nodes,
            tcp_port,
        })
    }
    
    /// Validate configuration values
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.block_width > 1000 || self.block_height > 1000 {
            anyhow::bail!("Block dimensions must not exceed 1000x1000 pixels");
        }
        
        if self.total_blocks == 0 {
            anyhow::bail!("Total blocks must be greater than 0");
        }
        
        if self.total_inodes == 0 {
            anyhow::bail!("Total inodes must be greater than 0");
        }
        
        Ok(())
    }
}
