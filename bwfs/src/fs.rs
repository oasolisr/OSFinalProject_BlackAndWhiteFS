use crate::inode::{DirEntry, FileType, INode};
use crate::storage::{Bitmap, BlockStorage};
use crate::config::Config;
use fuser::{
    FileAttr, FileType as FuseFileType, Filesystem, KernelConfig, ReplyAttr, ReplyData,
    ReplyDirectory, ReplyEntry, ReplyOpen, ReplyWrite, Request, ReplyCreate,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use anyhow::Result;

const TTL: Duration = Duration::from_secs(1);

/// Filesystem metadata for persistence
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FilesystemMetadata {
    inodes: HashMap<u64, INode>,
    directories: HashMap<u64, Vec<DirEntry>>,
    block_bitmap: Bitmap,
    inode_bitmap: Bitmap,
    next_ino: u64,
}

/// Main BWFS filesystem structure
pub struct BWFS {
    /// Block storage layer
    storage: Arc<Mutex<BlockStorage>>,
    
    /// INode table (in-memory cache)
    inodes: Arc<Mutex<HashMap<u64, INode>>>,
    
    /// Directory entries (ino -> Vec<DirEntry>)
    directories: Arc<Mutex<HashMap<u64, Vec<DirEntry>>>>,
    
    /// Open file handles (handle -> ino)
    open_files: Arc<Mutex<HashMap<u64, u64>>>,
    
    /// Next available file handle
    next_fh: Arc<Mutex<u64>>,
    
    /// Block bitmap
    block_bitmap: Arc<Mutex<Bitmap>>,
    
    /// INode bitmap
    inode_bitmap: Arc<Mutex<Bitmap>>,
    
    /// Configuration
    config: Config,
    
    /// Next available inode number
    next_ino: Arc<Mutex<u64>>,
}

impl BWFS {
    /// Create a new BWFS instance
    pub fn new(config: Config) -> Result<Self> {
        let storage = BlockStorage::new(
            &config.storage_path,
            config.block_width,
            config.block_height,
            config.total_blocks,
            config.fingerprint.clone(),
        )?;
        
        let block_bitmap = Bitmap::new(config.total_blocks as usize);
        let inode_bitmap = Bitmap::new(config.total_inodes as usize);
        
        let mut inodes = HashMap::new();
        let mut directories = HashMap::new();
        
        // Create root inode (ino = 1)
        let root_inode = INode::new(1, FileType::Directory, 0o755, 0, 0);
        inodes.insert(1, root_inode);
        
        // Create root directory entries (. and ..)
        directories.insert(1, vec![
            DirEntry::new(1, ".".to_string(), FileType::Directory),
            DirEntry::new(1, "..".to_string(), FileType::Directory),
        ]);
        
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
            inodes: Arc::new(Mutex::new(inodes)),
            directories: Arc::new(Mutex::new(directories)),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            next_fh: Arc::new(Mutex::new(1)),
            block_bitmap: Arc::new(Mutex::new(block_bitmap)),
            inode_bitmap: Arc::new(Mutex::new(inode_bitmap)),
            config,
            next_ino: Arc::new(Mutex::new(2)),
        })
    }
    
    /// Load existing filesystem
    pub fn load(config: Config) -> Result<Self> {
        use std::fs;
        use std::path::PathBuf;
        
        let storage = BlockStorage::new(
            &config.storage_path,
            config.block_width,
            config.block_height,
            config.total_blocks,
            config.fingerprint.clone(),
        )?;
        
        // Try to load metadata from block 1
        let metadata_path = PathBuf::from(&config.storage_path).join("metadata.json");
        
        if metadata_path.exists() {
            // Load from metadata file
            let metadata_str = fs::read_to_string(&metadata_path)?;
            let metadata: FilesystemMetadata = serde_json::from_str(&metadata_str)?;
            
            let inodes = metadata.inodes.into_iter().collect();
            let directories = metadata.directories.into_iter().collect();
            let next_ino = metadata.next_ino;
            
            Ok(Self {
                storage: Arc::new(Mutex::new(storage)),
                inodes: Arc::new(Mutex::new(inodes)),
                directories: Arc::new(Mutex::new(directories)),
                open_files: Arc::new(Mutex::new(HashMap::new())),
                next_fh: Arc::new(Mutex::new(1)),
                block_bitmap: Arc::new(Mutex::new(metadata.block_bitmap)),
                inode_bitmap: Arc::new(Mutex::new(metadata.inode_bitmap)),
                config,
                next_ino: Arc::new(Mutex::new(next_ino)),
            })
        } else {
            // Create new filesystem
            Self::new(config)
        }
    }
    
    /// Save filesystem state to disk
    pub fn save(&self) -> Result<()> {
        use std::fs;
        use std::path::PathBuf;
        
        let metadata = FilesystemMetadata {
            inodes: self.inodes.lock().unwrap().clone(),
            directories: self.directories.lock().unwrap().clone(),
            block_bitmap: self.block_bitmap.lock().unwrap().clone(),
            inode_bitmap: self.inode_bitmap.lock().unwrap().clone(),
            next_ino: *self.next_ino.lock().unwrap(),
        };
        
        let metadata_path = PathBuf::from(&self.config.storage_path).join("metadata.json");
        let metadata_str = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, metadata_str)?;
        
        Ok(())
    }
    
    /// Convert INode to FUSE FileAttr
    fn inode_to_attr(&self, inode: &INode) -> FileAttr {
        let kind = match inode.file_type {
            FileType::RegularFile => FuseFileType::RegularFile,
            FileType::Directory => FuseFileType::Directory,
            FileType::Symlink => FuseFileType::Symlink,
        };
        
        FileAttr {
            ino: inode.ino,
            size: inode.size,
            blocks: (inode.size + 511) / 512,
            atime: inode.atime,
            mtime: inode.mtime,
            ctime: inode.ctime,
            crtime: inode.ctime,
            kind,
            perm: inode.mode,
            nlink: inode.nlink,
            uid: inode.uid,
            gid: inode.gid,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }
    
    /// Allocate a new inode number
    fn allocate_ino(&self) -> u64 {
        let mut next_ino = self.next_ino.lock().unwrap();
        let ino = *next_ino;
        *next_ino += 1;
        
        let mut bitmap = self.inode_bitmap.lock().unwrap();
        bitmap.set(ino as usize);
        
        ino
    }
    
    /// Allocate a new file handle
    fn allocate_fh(&self) -> u64 {
        let mut next_fh = self.next_fh.lock().unwrap();
        let fh = *next_fh;
        *next_fh += 1;
        fh
    }
    
    /// Allocate a new block
    fn allocate_block(&self) -> Option<u32> {
        let mut bitmap = self.block_bitmap.lock().unwrap();
        bitmap.allocate().map(|idx| idx as u32)
    }
    
    /// Free a block
    fn free_block(&self, block_num: u32) {
        let mut bitmap = self.block_bitmap.lock().unwrap();
        bitmap.deallocate(block_num as usize);
    }
}

impl Filesystem for BWFS {
    fn init(&mut self, _req: &Request, _config: &mut KernelConfig) -> Result<(), libc::c_int> {
        log::info!("BWFS filesystem initialized");
        Ok(())
    }
    
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_string_lossy().to_string();
        log::debug!("lookup: parent={}, name={}", parent, name);
        
        let directories = self.directories.lock().unwrap();
        let inodes = self.inodes.lock().unwrap();
        
        if let Some(entries) = directories.get(&parent) {
            if let Some(entry) = entries.iter().find(|e| e.name == name) {
                if let Some(inode) = inodes.get(&entry.ino) {
                    let attr = self.inode_to_attr(inode);
                    reply.entry(&TTL, &attr, 0);
                    return;
                }
            }
        }
        
        reply.error(libc::ENOENT);
    }
    
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        log::debug!("getattr: ino={}", ino);
        
        let inodes = self.inodes.lock().unwrap();
        
        if let Some(inode) = inodes.get(&ino) {
            let attr = self.inode_to_attr(inode);
            reply.attr(&TTL, &attr);
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        log::debug!("open: ino={}, flags={}", ino, flags);
        
        let inodes = self.inodes.lock().unwrap();
        
        if inodes.contains_key(&ino) {
            let fh = self.allocate_fh();
            let mut open_files = self.open_files.lock().unwrap();
            open_files.insert(fh, ino);
            
            reply.opened(fh, 0);
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        log::debug!("read: ino={}, offset={}, size={}", ino, offset, size);
        
        let inodes = self.inodes.lock().unwrap();
        let storage = self.storage.lock().unwrap();
        
        if let Some(inode) = inodes.get(&ino) {
            if !inode.is_file() {
                reply.error(libc::EISDIR);
                return;
            }
            
            let mut data = Vec::new();
            let block_size = storage.bytes_per_block();
            let start_block = (offset as usize) / block_size;
            let end_block = ((offset as usize + size as usize) + block_size - 1) / block_size;
            
            for block_idx in start_block..end_block {
                if let Some(block_num) = inode.get_block_number(block_idx as u32) {
                    if let Ok(block_data) = storage.read_block(block_num) {
                        data.extend_from_slice(&block_data);
                    }
                }
            }
            
            let start_offset = (offset as usize) % block_size;
            let end_offset = start_offset + size as usize;
            let end_offset = end_offset.min(data.len());
            
            if start_offset < data.len() {
                reply.data(&data[start_offset..end_offset]);
            } else {
                reply.data(&[]);
            }
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        log::debug!("write: ino={}, offset={}, size={}", ino, offset, data.len());
        
        let mut inodes = self.inodes.lock().unwrap();
        let storage = self.storage.lock().unwrap();
        
        if let Some(inode) = inodes.get_mut(&ino) {
            if !inode.is_file() {
                reply.error(libc::EISDIR);
                return;
            }
            
            let block_size = storage.bytes_per_block();
            let start_block = (offset as usize) / block_size;
            let blocks_needed = ((offset as usize + data.len()) + block_size - 1) / block_size;
            
            // Allocate blocks if needed
            for block_idx in start_block..blocks_needed {
                if inode.get_block_number(block_idx as u32).is_none() {
                    if let Some(new_block) = self.allocate_block() {
                        inode.set_block_number(block_idx as u32, new_block);
                        let _ = storage.init_block(new_block);
                    } else {
                        reply.error(libc::ENOSPC);
                        return;
                    }
                }
            }
            
            // Write data to blocks
            let mut written = 0;
            for block_idx in start_block..blocks_needed {
                if let Some(block_num) = inode.get_block_number(block_idx as u32) {
                    let block_offset = if block_idx == start_block {
                        (offset as usize) % block_size
                    } else {
                        0
                    };
                    
                    let write_size = (block_size - block_offset).min(data.len() - written);
                    
                    // Read existing block data
                    let mut block_data = storage.read_block(block_num).unwrap_or_else(|_| vec![0; block_size]);
                    
                    // Update with new data
                    block_data[block_offset..block_offset + write_size]
                        .copy_from_slice(&data[written..written + write_size]);
                    
                    // Write back to disk
                    if let Err(e) = storage.write_block(block_num, &block_data) {
                        log::error!("Failed to write block: {}", e);
                        reply.error(libc::EIO);
                        return;
                    }
                    
                    written += write_size;
                }
            }
            
            // Update inode size and mtime
            let new_size = (offset as u64 + data.len() as u64).max(inode.size);
            inode.size = new_size;
            inode.mtime = SystemTime::now();
            
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn create(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let name = name.to_string_lossy().to_string();
        log::debug!("create: parent={}, name={}, mode={}", parent, name, mode);
        
        let mut inodes = self.inodes.lock().unwrap();
        let mut directories = self.directories.lock().unwrap();
        
        // Check if parent exists and is a directory
        if !inodes.get(&parent).map(|i| i.is_dir()).unwrap_or(false) {
            reply.error(libc::ENOTDIR);
            return;
        }
        
        // Check if file already exists
        if let Some(entries) = directories.get(&parent) {
            if entries.iter().any(|e| e.name == name) {
                reply.error(libc::EEXIST);
                return;
            }
        }
        
        // Create new inode
        let ino = self.allocate_ino();
        let inode = INode::new(ino, FileType::RegularFile, mode as u16, req.uid(), req.gid());
        let attr = self.inode_to_attr(&inode);
        
        inodes.insert(ino, inode);
        
        // Add to parent directory
        directories.entry(parent).or_insert_with(Vec::new)
            .push(DirEntry::new(ino, name, FileType::RegularFile));
        
        // ReplyCreate expects (ttl, attr, generation, fh, flags)
        let fh = self.allocate_fh();
        let mut open_files = self.open_files.lock().unwrap();
        open_files.insert(fh, ino);
        drop(open_files);
        
        reply.created(&TTL, &attr, 0, fh, 0);
    }
    
    fn mkdir(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name = name.to_string_lossy().to_string();
        log::debug!("mkdir: parent={}, name={}, mode={}", parent, name, mode);
        
        let mut inodes = self.inodes.lock().unwrap();
        let mut directories = self.directories.lock().unwrap();
        
        // Check if parent exists and is a directory
        if !inodes.get(&parent).map(|i| i.is_dir()).unwrap_or(false) {
            reply.error(libc::ENOTDIR);
            return;
        }
        
        // Check if directory already exists
        if let Some(entries) = directories.get(&parent) {
            if entries.iter().any(|e| e.name == name) {
                reply.error(libc::EEXIST);
                return;
            }
        }
        
        // Create new directory inode
        let ino = self.allocate_ino();
        let mut inode = INode::new(ino, FileType::Directory, mode as u16, req.uid(), req.gid());
        inode.nlink = 2; // . and ..
        let attr = self.inode_to_attr(&inode);
        
        inodes.insert(ino, inode);
        
        // Create directory entries (. and ..)
        directories.insert(ino, vec![
            DirEntry::new(ino, ".".to_string(), FileType::Directory),
            DirEntry::new(parent, "..".to_string(), FileType::Directory),
        ]);
        
        // Add to parent directory
        directories.entry(parent).or_insert_with(Vec::new)
            .push(DirEntry::new(ino, name, FileType::Directory));
        
        // Increment parent nlink
        if let Some(parent_inode) = inodes.get_mut(&parent) {
            parent_inode.nlink += 1;
        }
        
        reply.entry(&TTL, &attr, 0);
    }
    
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        log::debug!("readdir: ino={}, offset={}", ino, offset);
        
        let directories = self.directories.lock().unwrap();
        let _inodes = self.inodes.lock().unwrap();
        
        if let Some(entries) = directories.get(&ino) {
            for (i, entry) in entries.iter().enumerate().skip(offset as usize) {
                let kind = match entry.file_type {
                    FileType::RegularFile => FuseFileType::RegularFile,
                    FileType::Directory => FuseFileType::Directory,
                    FileType::Symlink => FuseFileType::Symlink,
                };
                
                let full = reply.add(entry.ino, (i + 1) as i64, kind, &entry.name);
                if full {
                    break;
                }
            }
        }
        
        reply.ok();
    }
    
    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        let name = name.to_string_lossy().to_string();
        log::debug!("unlink: parent={}, name={}", parent, name);
        
        let mut inodes = self.inodes.lock().unwrap();
        let mut directories = self.directories.lock().unwrap();
        
        if let Some(entries) = directories.get_mut(&parent) {
            if let Some(pos) = entries.iter().position(|e| e.name == name) {
                let entry = entries.remove(pos);
                
                // Decrease nlink and remove inode if nlink reaches 0
                if let Some(inode) = inodes.get_mut(&entry.ino) {
                    inode.nlink -= 1;
                    if inode.nlink == 0 {
                        // Free all blocks
                        for i in 0..12 {
                            if let Some(block_num) = inode.get_block_number(i) {
                                self.free_block(block_num);
                            }
                        }
                        inodes.remove(&entry.ino);
                    }
                }
                
                reply.ok();
                return;
            }
        }
        
        reply.error(libc::ENOENT);
    }
    
    fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        let name = name.to_string_lossy().to_string();
        log::debug!("rmdir: parent={}, name={}", parent, name);
        
        let mut inodes = self.inodes.lock().unwrap();
        let mut directories = self.directories.lock().unwrap();
        
        // First, find the entry and check if it's empty
        let entry_to_remove = if let Some(entries) = directories.get(&parent) {
            if let Some(entry) = entries.iter().find(|e| e.name == name && e.file_type == FileType::Directory) {
                // Check if directory is empty (only . and ..)
                if let Some(dir_entries) = directories.get(&entry.ino) {
                    if dir_entries.len() > 2 {
                        reply.error(libc::ENOTEMPTY);
                        return;
                    }
                }
                Some(entry.clone())
            } else {
                None
            }
        } else {
            None
        };
        
        // Now remove it
        if let Some(entry) = entry_to_remove {
            if let Some(entries) = directories.get_mut(&parent) {
                entries.retain(|e| e.ino != entry.ino);
            }
            directories.remove(&entry.ino);
            inodes.remove(&entry.ino);
            
            // Decrement parent nlink
            if let Some(parent_inode) = inodes.get_mut(&parent) {
                parent_inode.nlink -= 1;
            }
            
            reply.ok();
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn rename(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: fuser::ReplyEmpty,
    ) {
        let name = name.to_string_lossy().to_string();
        let newname = newname.to_string_lossy().to_string();
        log::debug!("rename: parent={}, name={}, newparent={}, newname={}", parent, name, newparent, newname);
        
        let mut directories = self.directories.lock().unwrap();
        
        // Find the entry in the old parent
        if let Some(entries) = directories.get_mut(&parent) {
            if let Some(pos) = entries.iter().position(|e| e.name == name) {
                let mut entry = entries.remove(pos);
                entry.name = newname.clone();
                
                // Add to new parent
                directories.entry(newparent).or_insert_with(Vec::new).push(entry);
                
                reply.ok();
                return;
            }
        }
        
        reply.error(libc::ENOENT);
    }
    
    fn flush(&mut self, _req: &Request, ino: u64, fh: u64, _lock_owner: u64, reply: fuser::ReplyEmpty) {
        log::debug!("flush: ino={}, fh={}", ino, fh);
        // Flush is called when a file descriptor is closed
        // For simplicity, we'll just return success
        reply.ok();
    }
    
    fn fsync(&mut self, _req: &Request, ino: u64, fh: u64, datasync: bool, reply: fuser::ReplyEmpty) {
        log::debug!("fsync: ino={}, fh={}, datasync={}", ino, fh, datasync);
        // Sync filesystem state to disk
        if let Err(e) = self.save() {
            log::error!("Failed to save filesystem: {}", e);
            reply.error(libc::EIO);
        } else {
            reply.ok();
        }
    }
    
    fn access(&mut self, _req: &Request, ino: u64, mask: i32, reply: fuser::ReplyEmpty) {
        log::debug!("access: ino={}, mask={}", ino, mask);
        
        let inodes = self.inodes.lock().unwrap();
        
        if inodes.contains_key(&ino) {
            // For simplicity, always grant access
            reply.ok();
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn statfs(&mut self, _req: &Request, ino: u64, reply: fuser::ReplyStatfs) {
        log::debug!("statfs: ino={}", ino);
        
        let block_bitmap = self.block_bitmap.lock().unwrap();
        let _inode_bitmap = self.inode_bitmap.lock().unwrap();
        
        let block_size = self.storage.lock().unwrap().bytes_per_block() as u32;
        let total_blocks = self.config.total_blocks as u64;
        
        // Count free blocks (simple approximation)
        let mut free_blocks = 0u64;
        for i in 0..self.config.total_blocks as usize {
            if !block_bitmap.is_set(i) {
                free_blocks += 1;
            }
        }
        
        reply.statfs(
            total_blocks,           // blocks
            free_blocks,            // bfree
            free_blocks,            // bavail
            self.config.total_inodes as u64,  // files
            self.config.total_inodes as u64 - self.inodes.lock().unwrap().len() as u64,  // ffree
            block_size,             // bsize
            255,                    // namelen
            block_size,             // frsize
        );
    }
    
    fn opendir(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        log::debug!("opendir: ino={}, flags={}", ino, flags);
        
        let inodes = self.inodes.lock().unwrap();
        
        if let Some(inode) = inodes.get(&ino) {
            if inode.is_dir() {
                let fh = self.allocate_fh();
                reply.opened(fh, 0);
            } else {
                reply.error(libc::ENOTDIR);
            }
        } else {
            reply.error(libc::ENOENT);
        }
    }
    
    fn release(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: fuser::ReplyEmpty,
    ) {
        log::debug!("release: ino={}, fh={}", ino, fh);
        
        let mut open_files = self.open_files.lock().unwrap();
        open_files.remove(&fh);
        
        reply.ok();
    }
    
    fn releasedir(&mut self, _req: &Request, ino: u64, fh: u64, _flags: i32, reply: fuser::ReplyEmpty) {
        log::debug!("releasedir: ino={}, fh={}", ino, fh);
        
        let mut open_files = self.open_files.lock().unwrap();
        open_files.remove(&fh);
        
        reply.ok();
    }
}
