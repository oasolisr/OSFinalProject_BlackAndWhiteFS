use clap::Parser;
use bwfs::{Config, BWFS};
use anyhow::Result;
use fuser::MountOption;
use std::path::Path;

/// mount.bwfs - Mount a BWFS filesystem
#[derive(Parser, Debug)]
#[command(name = "mount.bwfs")]
#[command(about = "Mount a BWFS (Black and White FileSystem)", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short = 'c', long = "config")]
    config: String,
    
    /// Mount point directory
    #[arg(value_name = "MOUNTPOINT")]
    mountpoint: String,
    
    /// Allow other users to access the filesystem
    #[arg(short = 'o', long = "allow-other")]
    allow_other: bool,
    
    /// Run in foreground
    #[arg(short = 'f', long = "foreground")]
    foreground: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    
    let args = Args::parse();
    
    println!("mount.bwfs - Mounting Black and White FileSystem");
    println!("=================================================");
    
    // Load configuration
    println!("Loading configuration from: {}", args.config);
    let config = Config::from_ini(&args.config)?;
    
    // Validate configuration
    config.validate()?;
    
    println!("Filesystem name: {}", config.name);
    println!("Storage path: {}", config.storage_path);
    println!("Mount point: {}", args.mountpoint);
    
    // Check if storage path exists
    let storage_path = Path::new(&config.storage_path);
    if !storage_path.exists() {
        anyhow::bail!("Storage path does not exist. Did you run mkfs.bwfs?");
    }
    
    // Verify fingerprint
    println!("Verifying filesystem fingerprint...");
    let storage = bwfs::storage::BlockStorage::new(
        &config.storage_path,
        config.block_width,
        config.block_height,
        config.total_blocks,
        config.fingerprint.clone(),
    )?;
    
    if !storage.verify_fingerprint()? {
        anyhow::bail!("Filesystem fingerprint mismatch! This may not be a valid BWFS.");
    }
    
    println!("✓ Fingerprint verified");
    
    // Load or create filesystem
    println!("Loading filesystem...");
    let fs = BWFS::load(config.clone())
        .or_else(|_| {
            println!("Creating new filesystem instance...");
            BWFS::new(config.clone())
        })?;
    
    // Prepare mount options
    let mut options = vec![
        MountOption::FSName("bwfs".to_string()),
        MountOption::RW,
    ];
    
    if args.allow_other {
        options.push(MountOption::AllowOther);
    }
    
    if !args.foreground {
        println!("\nMounting filesystem in background...");
        println!("To unmount, use: fusermount -u {}", args.mountpoint);
    } else {
        println!("\nMounting filesystem in foreground...");
        println!("Press Ctrl+C to unmount");
    }
    
    // Mount the filesystem
    println!("✓ Mounting at {}", args.mountpoint);
    fuser::mount2(fs, args.mountpoint, &options)?;
    
    Ok(())
}
