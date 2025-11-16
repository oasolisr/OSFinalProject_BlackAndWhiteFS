use clap::Parser;
use bwfs::{Config, BWFS};
use anyhow::Result;

/// mkfs.bwfs - Create a new BWFS filesystem
#[derive(Parser, Debug)]
#[command(name = "mkfs.bwfs")]
#[command(about = "Create a new BWFS (Black and White FileSystem)", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short = 'c', long = "config")]
    config: String,
}

fn main() -> Result<()> {
    env_logger::init();
    
    let args = Args::parse();
    
    println!("mkfs.bwfs - Creating Black and White FileSystem");
    println!("================================================");
    
    // Load configuration
    println!("Loading configuration from: {}", args.config);
    let config = Config::from_ini(&args.config)?;
    
    // Validate configuration
    println!("Validating configuration...");
    config.validate()?;
    
    println!("Filesystem name: {}", config.name);
    println!("Block dimensions: {}x{} pixels", config.block_width, config.block_height);
    println!("Total blocks: {}", config.total_blocks);
    println!("Total inodes: {}", config.total_inodes);
    println!("Storage path: {}", config.storage_path);
    println!("Fingerprint: {}", config.fingerprint);
    
    // Calculate filesystem capacity
    let bytes_per_block = (config.block_width * config.block_height / 8) as u64;
    let total_capacity = bytes_per_block * config.total_blocks as u64;
    let capacity_mb = total_capacity as f64 / (1024.0 * 1024.0);
    
    println!("Bytes per block: {}", bytes_per_block);
    println!("Total capacity: {:.2} MB", capacity_mb);
    
    // Create the filesystem
    println!("\nCreating filesystem structure...");
    let fs = BWFS::new(config.clone())?;
    
    // Initialize storage
    println!("Initializing block storage...");
    let storage = bwfs::storage::BlockStorage::new(
        &config.storage_path,
        config.block_width,
        config.block_height,
        config.total_blocks,
        config.fingerprint.clone(),
    )?;
    
    // Initialize first few blocks
    println!("Initializing system blocks...");
    for i in 0..10.min(config.total_blocks) {
        storage.init_block(i)?;
        if i % 10 == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout())?;
        }
    }
    println!();
    
    // Write fingerprint to superblock (block 0) - AFTER initializing
    println!("Writing fingerprint to superblock...");
    storage.write_fingerprint()?;
    
    // Save filesystem metadata
    println!("Saving filesystem metadata...");
    fs.save()?;
    
    println!("\n✓ Filesystem created successfully!");
    println!("You can now mount it using: mount.bwfs -c {} <mountpoint>", args.config);
    
    Ok(())
}
