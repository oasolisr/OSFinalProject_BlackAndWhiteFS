use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// File types supported by BWFS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    RegularFile,
    Directory,
    Symlink,
}

/// INode structure for BWFS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct INode {
    /// Unique inode number
    pub ino: u64,
    
    /// File type
    pub file_type: FileType,
    
    /// File size in bytes
    pub size: u64,
    
    /// Number of hard links
    pub nlink: u32,
    
    /// User ID
    pub uid: u32,
    
    /// Group ID
    pub gid: u32,
    
    /// Permissions (mode)
    pub mode: u16,
    
    /// Access time
    pub atime: SystemTime,
    
    /// Modification time
    pub mtime: SystemTime,
    
    /// Change time
    pub ctime: SystemTime,
    
    /// Direct block pointers (block numbers)
    pub direct_blocks: [u32; 12],
    
    /// Single indirect block pointer
    pub indirect_block: u32,
    
    /// Double indirect block pointer
    pub double_indirect_block: u32,
}

impl INode {
    /// Create a new inode
    pub fn new(ino: u64, file_type: FileType, mode: u16, uid: u32, gid: u32) -> Self {
        let now = SystemTime::now();
        
        Self {
            ino,
            file_type,
            size: 0,
            nlink: 1,
            uid,
            gid,
            mode,
            atime: now,
            mtime: now,
            ctime: now,
            direct_blocks: [u32::MAX; 12],
            indirect_block: u32::MAX,
            double_indirect_block: u32::MAX,
        }
    }
    
    /// Check if this is a directory
    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }
    
    /// Check if this is a regular file
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::RegularFile
    }
    
    /// Get block number for a given file offset
    pub fn get_block_number(&self, block_index: u32) -> Option<u32> {
        if block_index < 12 {
            let block = self.direct_blocks[block_index as usize];
            if block != u32::MAX {
                Some(block)
            } else {
                None
            }
        } else {
            // TODO: Implement indirect block logic
            None
        }
    }
    
    /// Set block number for a given file offset
    pub fn set_block_number(&mut self, block_index: u32, block_num: u32) -> bool {
        if block_index < 12 {
            self.direct_blocks[block_index as usize] = block_num;
            true
        } else {
            // TODO: Implement indirect block logic
            false
        }
    }
}

/// Directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    /// Inode number
    pub ino: u64,
    
    /// File name
    pub name: String,
    
    /// File type
    pub file_type: FileType,
}

impl DirEntry {
    pub fn new(ino: u64, name: String, file_type: FileType) -> Self {
        Self {
            ino,
            name,
            file_type,
        }
    }
}
