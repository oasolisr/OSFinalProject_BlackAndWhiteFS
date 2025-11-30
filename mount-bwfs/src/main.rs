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
    let mut config = Config::from_ini(&args.config)?;
    
    // Trim fingerprint (avoid mismatch caused by trailing spaces/newlines)
    config.fingerprint = config.fingerprint.trim().to_string();
    
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
    
    // ==================================================================
    // DEBUG: LEER LA PRIMERA PARTE DEL BLOQUE 0 PARA VER EL FINGERPRINT
    // ==================================================================
    let block0 = storage.read_block(0)?;
    let fp = config.fingerprint.as_bytes();

    println!("[DEBUG] First 32 bytes of block 0 (ASCII): {:?}",
            String::from_utf8_lossy(&block0[..32]));

    println!("[DEBUG] Expected FP bytes: {:?}", fp);
    println!("[DEBUG] Found FP bytes   : {:?}", &block0[..fp.len()]);

    // Comprobación manual antes del verify_fingerprint()
    if !block0.starts_with(fp) {
        println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        println!("FINGERPRINT MISMATCH BEFORE MOUNTING");
        println!("Expected: {}", config.fingerprint);
        println!("Found (ASCII): {:?}", String::from_utf8_lossy(&block0[..fp.len()]));
        println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\n");
    }

    match storage.verify_fingerprint() {
        Ok(true) => println!("✓ Fingerprint verified"),
        Ok(false) => {
            anyhow::bail!(
                "Filesystem fingerprint mismatch!\n\
                 Expected: '{}'\n\
                 But block 0 does NOT begin with that fingerprint.\n\
                 Possible causes:\n\
                   - mkfs_bwfs did not write the fingerprint.\n\
                   - block_00000000.png was overwritten or corrupted.\n\
                   - fingerprint in config.ini contains hidden spaces.",
                config.fingerprint
            );
        }
        Err(e) => {
            anyhow::bail!("Error reading fingerprint from block 0: {}", e);
        }
    }
    
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
