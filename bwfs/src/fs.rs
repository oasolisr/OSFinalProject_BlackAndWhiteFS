use crate::inode::{DirEntry, FileType, INode};
use crate::storage::{Bitmap, BlockStorage};
use crate::config::Config;
use fuser::{
    FileAttr, FileType as FuseFileType, Filesystem, KernelConfig, ReplyAttr, ReplyData,
    ReplyDirectory, ReplyEntry, ReplyOpen, ReplyWrite, Request, ReplyCreate, ReplyEmpty, ReplyStatfs,
};
use std::collections::{HashMap};
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

    /// Global dirty flag: true if metadata (inodes/dirs/bitmaps) has pending changes
    dirty: Arc<Mutex<bool>>,
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

        // Bitmap de bloques: todos libres al inicio.
        // Reservamos explícitamente el bloque 0 para el superblock/fingerprint.
        let mut block_bitmap = Bitmap::new(config.total_blocks as usize);
        block_bitmap.set(0); // 🔒 bloque 0 reservado (superblock)

        let inode_bitmap = Bitmap::new(config.total_inodes as usize);

        let mut inodes = HashMap::new();
        let mut directories = HashMap::new();

        // Create root inode (ino = 1)
        let root_inode = INode::new(1, FileType::Directory, 0o755, 0, 0);
        inodes.insert(1, root_inode);

        // Create root directory entries (. and ..)
        directories.insert(
            1,
            vec![
                DirEntry::new(1, ".".to_string(), FileType::Directory),
                DirEntry::new(1, "..".to_string(), FileType::Directory),
            ],
        );

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
            dirty: Arc::new(Mutex::new(false)),
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

        // Try to load metadata from metadata.json
        let metadata_path = PathBuf::from(&config.storage_path).join("metadata.json");

        if metadata_path.exists() {
            // Load from metadata file
            let metadata_str = fs::read_to_string(&metadata_path)?;
            let metadata: FilesystemMetadata = serde_json::from_str(&metadata_str)?;

            let inodes = metadata.inodes.into_iter().collect();
            let directories = metadata.directories.into_iter().collect();
            let next_ino = metadata.next_ino;

            // Aseguramos que el bloque 0 SIEMPRE quede reservado,
            // aunque una versión vieja del FS no lo tuviera marcado.
            let mut bb = metadata.block_bitmap.clone();
            bb.set(0); // 🔒 bloque 0 reservado (superblock)

            Ok(Self {
                storage: Arc::new(Mutex::new(storage)),
                inodes: Arc::new(Mutex::new(inodes)),
                directories: Arc::new(Mutex::new(directories)),
                open_files: Arc::new(Mutex::new(HashMap::new())),
                next_fh: Arc::new(Mutex::new(1)),
                block_bitmap: Arc::new(Mutex::new(bb)),
                inode_bitmap: Arc::new(Mutex::new(metadata.inode_bitmap)),
                config,
                next_ino: Arc::new(Mutex::new(next_ino)),
                dirty: Arc::new(Mutex::new(false)),
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

        log::info!("BWFS::save() -> escribiendo metadata.json en disco");

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

        log::info!(
            "BWFS::save() -> metadata.json actualizado en {:?}",
            metadata_path
        );

        Ok(())
    }

    /// Marca el filesystem como "sucio" (con cambios pendientes de persistir)
    fn mark_dirty(&self) {
        let mut dirty = self.dirty.lock().unwrap();
        *dirty = true;
        log::info!("📌 mark_dirty(): filesystem marcado como DIRTY");
    }

    /// Si hay cambios pendientes, llama a `save()` y limpia la bandera.
    fn sync_if_dirty(&self) -> Result<()> {
        {
            let dirty = self.dirty.lock().unwrap();
            if !*dirty {
                log::info!("📌 sync_if_dirty(): metadata CLEAN, nada que sincronizar");
                return Ok(());
            }
        }

        log::info!("📌 sync_if_dirty(): metadata DIRTY, llamando a save() ...");
        self.save()?;
        let mut dirty = self.dirty.lock().unwrap();
        *dirty = false;
        log::info!("📌 sync_if_dirty(): metadata sincronizada, bandera limpia");
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

    /// Allocate a new block (nunca retorna el bloque 0 porque está reservado en el bitmap)
    fn allocate_block(&self) -> Option<u32> {
        let mut bitmap = self.block_bitmap.lock().unwrap();
        bitmap.allocate().map(|idx| idx as u32)
    }

    /// Free a block
    fn free_block(&self, block_num: u32) {
        let mut bitmap = self.block_bitmap.lock().unwrap();
        // Nunca deberíamos liberar el bloque 0; por seguridad lo evitamos
        if block_num != 0 {
            bitmap.deallocate(block_num as usize);
        }
    }
}

macro_rules! log_enter {
    ($func:expr) => {
        log::info!("➡️ ENTER {}", $func);
    };
}

macro_rules! log_exit {
    ($func:expr) => {
        log::info!("⬅️ EXIT {}", $func);
    };
}

macro_rules! log_point {
    ($msg:expr) => {{
        log::info!("📌 {}", $msg);
    }};
}

impl Filesystem for BWFS {
    fn init(&mut self, _req: &Request, _config: &mut KernelConfig) -> Result<(), libc::c_int> {
        log_enter!("init()");
        log_point!("Initializing FS");
        log_exit!("init()");
        Ok(())
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &std::ffi::OsStr, reply: ReplyEntry) {
        let name = name.to_string_lossy().to_string();
        log_enter!("lookup()");
        log_point!(format!("lookup: parent={}, name={}", parent, name.clone()));

        let directories = self.directories.lock().unwrap();
        let inodes = self.inodes.lock().unwrap();

        if let Some(entries) = directories.get(&parent) {
            if let Some(entry) = entries.iter().find(|e| e.name == name) {
                log_point!("lookup match found");
                if let Some(inode) = inodes.get(&entry.ino) {
                    let attr = self.inode_to_attr(inode);
                    reply.entry(&TTL, &attr, 0);
                    log_exit!("lookup()");
                    return;
                }
            }
        }

        log_point!("lookup: NOENT");
        reply.error(libc::ENOENT);
        log_exit!("lookup()");
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        log_enter!("getattr()");
        log_point!(format!("getattr ino={}", ino));

        let inodes = self.inodes.lock().unwrap();

        if let Some(inode) = inodes.get(&ino) {
            let attr = self.inode_to_attr(inode);
            reply.attr(&TTL, &attr);
        } else {
            log_point!("getattr: NOENT");
            reply.error(libc::ENOENT);
        }
        log_exit!("getattr()");
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        log_enter!("open()");
        log_point!(format!("open ino={} flags={}", ino, flags));

        let inodes = self.inodes.lock().unwrap();

        if inodes.contains_key(&ino) {
            let fh = self.allocate_fh();
            let mut open_files = self.open_files.lock().unwrap();
            open_files.insert(fh, ino);

            log_point!(format!("open: fh={} assigned", fh));

            reply.opened(fh, 0);
        } else {
            log_point!("open: NOENT");
            reply.error(libc::ENOENT);
        }
        log_exit!("open()");
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
        log_point!(format!(
            "read: ino={}, offset={}, size={}",
            ino, offset, size
        ));

        let inodes = self.inodes.lock().unwrap();
        let storage = self.storage.lock().unwrap();

        if let Some(inode) = inodes.get(&ino) {
            if !inode.is_file() {
                log_point!("read -> EISDIR");
                reply.error(libc::EISDIR);
                return;
            }

            let mut data = Vec::new();
            let block_size = storage.bytes_per_block();
            log_point!(format!("read -> block_size={}", block_size));

            let start_block = (offset as usize) / block_size;
            let end_block = ((offset as usize + size as usize) + block_size - 1) / block_size;

            log_point!(format!(
                "read -> start_block={} end_block={}",
                start_block, end_block
            ));

            for block_idx in start_block..end_block {
                if let Some(block_num) = inode.get_block_number(block_idx as u32) {
                    log_point!(format!(
                        "read -> block {} mapped to physical {}",
                        block_idx, block_num
                    ));
                    if let Ok(block_data) = storage.read_block(block_num) {
                        data.extend_from_slice(&block_data);
                    } else {
                        log_point!(format!("read -> error reading block {}", block_num));
                    }
                } else {
                    log_point!(format!("read -> block {} not allocated", block_idx));
                }
            }

            let start_offset = (offset as usize) % block_size;
            let end_offset = (start_offset + size as usize).min(data.len());

            log_point!(format!(
                "read -> slicing data from {} to {} (data.len={})",
                start_offset,
                end_offset,
                data.len()
            ));

            if start_offset < data.len() {
                reply.data(&data[start_offset..end_offset]);
            } else {
                reply.data(&[]);
            }
        } else {
            log_point!("read -> ENOENT");
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
        log_point!(format!(
            "ENTER write(): ino={}, offset={}, size={}",
            ino,
            offset,
            data.len()
        ));

        // --------------------------------------------
        // BLOQUE DE LOCK → se libera al salir
        // --------------------------------------------
        let write_result = {
            let mut inodes = self.inodes.lock().unwrap();
            let storage = self.storage.lock().unwrap();

            // Obtener el inode
            let inode = match inodes.get_mut(&ino) {
                Some(inode) => inode,
                None => {
                    log_point!("write() -> ENOENT");
                    reply.error(libc::ENOENT);
                    return;
                }
            };

            if !inode.is_file() {
                log_point!("write() -> EISDIR");
                reply.error(libc::EISDIR);
                return;
            }

            let block_size = storage.bytes_per_block();
            log_point!(format!("write() -> block_size={}", block_size));

            let start_block = (offset as usize) / block_size;
            let blocks_needed = ((offset as usize + data.len()) + block_size - 1) / block_size;

            log_point!(format!(
                "write() -> start_block={} blocks_needed={}",
                start_block, blocks_needed
            ));

            // --------------------------------------------
            // Asignar bloques faltantes (usa allocate_block → safe)
            // --------------------------------------------
            for block_idx in start_block..blocks_needed {
                if inode.get_block_number(block_idx as u32).is_none() {
                    // Intentar asignar bloque
                    if let Some(new_block) = self.allocate_block() {
                        log_point!(format!(
                            "write() -> allocating PHYSICAL block {}",
                            new_block
                        ));

                        inode.set_block_number(block_idx as u32, new_block);

                        let _ = storage.init_block(new_block);
                    } else {
                        log_point!("write() -> ENOSPC");
                        reply.error(libc::ENOSPC);
                        return;
                    }
                }
            }

            // --------------------------------------------
            // Escribir datos
            // --------------------------------------------
            let mut written = 0;

            for block_idx in start_block..blocks_needed {
                let block_num = inode.get_block_number(block_idx as u32).unwrap();

                log_point!(format!("write() -> writing to block {}", block_num));

                let block_offset = if block_idx == start_block {
                    (offset as usize) % block_size
                } else {
                    0
                };

                let write_size = (block_size - block_offset).min(data.len() - written);

                let mut block_data =
                    storage.read_block(block_num).unwrap_or_else(|_| vec![0; block_size]);

                block_data[block_offset..block_offset + write_size]
                    .copy_from_slice(&data[written..written + write_size]);

                if let Err(e) = storage.write_block(block_num, &block_data) {
                    log_point!(format!("write() -> error writing block: {}", e));
                    reply.error(libc::EIO);
                    return;
                }

                written += write_size;

                log_point!(format!(
                    "write() -> wrote {} bytes into block {}",
                    write_size, block_num
                ));
            }

            // --------------------------------------------
            // Actualizar metadata del inode
            // --------------------------------------------
            let new_size = (offset as u64 + data.len() as u64).max(inode.size);
            log_point!(format!("write() -> new inode size={}", new_size));

            inode.size = new_size;
            inode.mtime = SystemTime::now();

            // Resultado a devolver luego fuera del lock
            data.len() as u32
        }; // <-- aquí se LIBERAN TODOS LOS LOCKS (inodes + storage)

        // Marcar metadata como sucia; se sincronizará en fsync()/release()
        self.mark_dirty();

        log_point!("write() -> EXIT OK (lazy metadata, fsync/release will persist)");
        reply.written(write_result);
    }

    fn create(
        &mut self,
        req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let name = name.to_string_lossy().to_string();
        log_point!(format!(
            "ENTER create(): parent={}, name='{}', mode={}",
            parent, name, mode
        ));

        // Vamos a devolver estos valores después del bloque de locks
        let (ino, attr, fh) = {
            log_point!("create() -> locking inodes and directories");
            let mut inodes = self.inodes.lock().unwrap();
            let mut directories = self.directories.lock().unwrap();
            log_point!("create() -> locks acquired");

            // --------------------------------------------
            // VALIDATE PARENT
            // --------------------------------------------
            if !inodes
                .get(&parent)
                .map(|i| i.is_dir())
                .unwrap_or(false)
            {
                log_point!(format!(
                    "create() -> ERROR: parent={} no es directorio",
                    parent
                ));
                reply.error(libc::ENOTDIR);
                log_exit!("create() -> exit ENOTDIR");
                return;
            }

            // --------------------------------------------
            // CHECK IF FILE ALREADY EXISTS
            // --------------------------------------------
            if let Some(entries) = directories.get(&parent) {
                if entries.iter().any(|e| e.name == name) {
                    log_point!(format!(
                        "create() -> ERROR: file '{}' already exists in parent {}",
                        name, parent
                    ));
                    reply.error(libc::EEXIST);
                    log_exit!("create() -> exit EEXIST");
                    return;
                }
            }

            // --------------------------------------------
            // ALLOCATE INODE
            // --------------------------------------------
            let ino = self.allocate_ino();
            log_point!(format!("create() -> allocated inode {}", ino));

            let inode = INode::new(
                ino,
                FileType::RegularFile,
                mode as u16,
                req.uid(),
                req.gid(),
            );
            let attr = self.inode_to_attr(&inode);

            inodes.insert(ino, inode);
            log_point!("create() -> inode inserted into inode table");

            // --------------------------------------------
            // ADD ENTRY TO PARENT DIRECTORY
            // --------------------------------------------
            directories
                .entry(parent)
                .or_insert_with(Vec::new)
                .push(DirEntry::new(ino, name.clone(), FileType::RegularFile));

            log_point!(format!(
                "create() -> Added DirEntry '{}' (ino={}) to parent {}",
                name, ino, parent
            ));

            // --------------------------------------------
            // ALLOCATE FILE HANDLE
            // --------------------------------------------
            let fh = self.allocate_fh();
            log_point!(format!("create() -> allocated file handle {}", fh));

            let mut open_files = self.open_files.lock().unwrap();
            open_files.insert(fh, ino);
            log_point!(format!(
                "create() -> open_files updated, fh={} -> ino={}",
                fh, ino
            ));

            (ino, attr, fh)
        };

        // Marcar metadata como sucia; se sincronizará en fsync()/release()
        self.mark_dirty();

        // --------------------------------------------
        // SEND REPLY
        // --------------------------------------------
        log_point!(format!(
            "create() -> replying created file: ino={}, fh={}",
            ino, fh
        ));
        reply.created(&TTL, &attr, 0, fh, 0);

        log_exit!("create() -> EXIT OK");
    }

    fn mkdir(
        &mut self,
        req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name = name.to_string_lossy().to_string();
        log_point!(format!(
            "ENTER mkdir(): parent={}, name='{}', mode={}",
            parent, name, mode
        ));

        // Vamos a construir estos valores mientras tenemos locks
        let (ino, attr, success) = {
            log_point!("mkdir() -> locking inodes and directories");
            let mut inodes = self.inodes.lock().unwrap();
            let mut directories = self.directories.lock().unwrap();
            log_point!("mkdir() -> locks acquired");

            // --------------------------------------------
            // VALIDATE PARENT DIRECTORY
            // --------------------------------------------
            if !inodes.get(&parent).map(|i| i.is_dir()).unwrap_or(false) {
                log_point!(format!(
                    "mkdir() -> ERROR: parent={} is not a directory",
                    parent
                ));
                reply.error(libc::ENOTDIR);
                log_exit!("mkdir() -> exit ENOTDIR");
                return;
            }

            // --------------------------------------------
            // CHECK FOR EXISTING NAME
            // --------------------------------------------
            if let Some(entries) = directories.get(&parent) {
                if entries.iter().any(|e| e.name == name) {
                    log_point!(format!(
                        "mkdir() -> ERROR: directory '{}' already exists in parent {}",
                        name, parent
                    ));
                    reply.error(libc::EEXIST);
                    log_exit!("mkdir() -> exit EEXIST");
                    return;
                }
            }

            // --------------------------------------------
            // ALLOCATE INODE FOR NEW DIRECTORY
            // --------------------------------------------
            let ino = self.allocate_ino();
            log_point!(format!("mkdir() -> allocated inode {}", ino));

            let mut inode = INode::new(
                ino,
                FileType::Directory,
                mode as u16,
                req.uid(),
                req.gid(),
            );
            inode.nlink = 2;

            let attr = self.inode_to_attr(&inode);

            inodes.insert(ino, inode);
            log_point!(format!("mkdir() -> inserted inode {} into inode table", ino));

            // --------------------------------------------
            // INSERT '.' and '..'
            // --------------------------------------------
            directories.insert(
                ino,
                vec![
                    DirEntry::new(ino, ".".to_string(), FileType::Directory),
                    DirEntry::new(parent, "..".to_string(), FileType::Directory),
                ],
            );

            log_point!(format!(
                "mkdir() -> created '.' and '..' entries for directory {}",
                ino
            ));

            // --------------------------------------------
            // ADD ENTRY IN PARENT DIRECTORY
            // --------------------------------------------
            directories
                .entry(parent)
                .or_insert_with(Vec::new)
                .push(DirEntry::new(ino, name.clone(), FileType::Directory));

            log_point!(format!(
                "mkdir() -> added '{}' (ino={}) to parent {}",
                name, ino, parent
            ));

            // --------------------------------------------
            // INCREMENT PARENT nlink
            // --------------------------------------------
            if let Some(parent_inode) = inodes.get_mut(&parent) {
                let old = parent_inode.nlink;
                parent_inode.nlink += 1;

                log_point!(format!(
                    "mkdir() -> parent {} nlink {} -> {}",
                    parent, old, parent_inode.nlink
                ));
            }

            (ino, attr, true)
            // <-- locks se liberan aquí porque salimos del bloque
        };

        if success {
            // Marcar metadata como sucia; se sincronizará en fsync()/release()
            self.mark_dirty();

            // --------------------------------------------
            // SEND REPLY
            // --------------------------------------------
            log_point!(format!("mkdir() -> replying entry: ino={}", ino));
            reply.entry(&TTL, &attr, 0);

            log_exit!("mkdir() -> EXIT OK");
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        log_point!(format!("ENTER readdir(): ino={}, offset={}", ino, offset));

        // --------------------------------------------
        // LOCK DIRECTORIES + INODES
        // --------------------------------------------
        log_point!("readdir() -> locking directories and inodes");
        let directories = self.directories.lock().unwrap();
        let _inodes = self.inodes.lock().unwrap();
        log_point!("readdir() -> locks acquired");

        // --------------------------------------------
        // GET DIRECTORY ENTRIES
        // --------------------------------------------
        if let Some(entries) = directories.get(&ino) {
            log_point!(format!(
                "readdir() -> directory {} has {} entries",
                ino,
                entries.len()
            ));

            // Iterate entries starting at offset
            for (i, entry) in entries.iter().enumerate().skip(offset as usize) {
                log_point!(format!(
                    "readdir() -> adding entry index={}, ino={}, name='{}'",
                    i, entry.ino, entry.name
                ));

                let kind = match entry.file_type {
                    FileType::RegularFile => FuseFileType::RegularFile,
                    FileType::Directory => FuseFileType::Directory,
                    FileType::Symlink => FuseFileType::Symlink,
                };

                let full = reply.add(entry.ino, (i + 1) as i64, kind, &entry.name);

                if full {
                    log_point!(format!(
                        "readdir() -> reply buffer FULL after entry index={} (ino={})",
                        i, entry.ino
                    ));
                    break;
                }
            }
        } else {
            log_point!(format!(
                "readdir() -> directory {} NOT FOUND in directories table",
                ino
            ));
        }

        // --------------------------------------------
        // SEND OK REPLY
        // --------------------------------------------
        log_point!("readdir() -> sending reply.ok()");
        reply.ok();

        log_exit!("readdir()");
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &std::ffi::OsStr, reply: ReplyEmpty) {
        let name = name.to_string_lossy().to_string();
        log_point!(format!("ENTER unlink(): parent={}, name={}", parent, name));

        let mut success = false;

        {
            // --------------------------------------------
            // LOCK INODES + DIRS
            // --------------------------------------------
            let mut inodes = self.inodes.lock().unwrap();
            let mut directories = self.directories.lock().unwrap();
            log_point!("unlink() -> locks acquired");

            // --------------------------------------------
            // Buscar entrada en el directorio padre
            // --------------------------------------------
            if let Some(entries) = directories.get_mut(&parent) {
                if let Some(pos) = entries.iter().position(|e| e.name == name) {
                    let entry = entries.remove(pos);
                    log_point!(format!("unlink(): removed DirEntry for ino={}", entry.ino));

                    // Reducir nlink
                    if let Some(inode) = inodes.get_mut(&entry.ino) {
                        let old = inode.nlink;
                        inode.nlink -= 1;
                        log_point!(format!(
                            "unlink(): inode {} nlink {} -> {}",
                            entry.ino, old, inode.nlink
                        ));

                        if inode.nlink == 0 {
                            log_point!(format!(
                                "unlink(): inode {} nlink=0 → freeing blocks",
                                entry.ino
                            ));

                            for i in 0..12 {
                                if let Some(block_num) = inode.get_block_number(i) {
                                    self.free_block(block_num);
                                    log_point!(format!("unlink(): freed block {}", block_num));
                                }
                            }

                            inodes.remove(&entry.ino);
                            log_point!(format!("unlink(): inode {} removed", entry.ino));
                        }
                    }

                    success = true;
                } else {
                    log_point!(format!(
                        "unlink(): entry '{}' not found in parent {}",
                        name, parent
                    ));
                }
            } else {
                log_point!(format!("unlink(): parent directory {} not found", parent));
            }
        } // <---- Locks se liberan aquí

        if success {
            // Directory tree cambió → metadata sucia
            self.mark_dirty();
            reply.ok();
            log_exit!("unlink() -> EXIT OK");
        } else {
            reply.error(libc::ENOENT);
            log_exit!("unlink() -> ENOENT");
        }
    }

    fn rmdir(&mut self, _req: &Request, parent: u64, name: &std::ffi::OsStr, reply: ReplyEmpty) {
        let name = name.to_string_lossy().to_string();
        log_point!(format!("ENTER rmdir(): parent={}, name={}", parent, name));

        // Variables de salida
        let mut exit_code: Option<i32> = None; // None = OK, Some(errno) = error

        {
            // --------------------------------------------
            // LOCK INODES + DIRECTORIES
            // --------------------------------------------
            let mut inodes = self.inodes.lock().unwrap();
            let mut directories = self.directories.lock().unwrap();
            log_point!("rmdir() -> locks acquired");

            // --------------------------------------------
            // Buscar el directorio
            // --------------------------------------------
            let entry_opt = directories
                .get(&parent)
                .and_then(|entries| {
                    entries
                        .iter()
                        .find(|e| e.name == name && e.file_type == FileType::Directory)
                        .cloned()
                });

            if entry_opt.is_none() {
                log_point!(format!(
                    "rmdir(): '{}' not found under parent {}",
                    name, parent
                ));
                exit_code = Some(libc::ENOENT);
            } else {
                let entry = entry_opt.unwrap();

                // --------------------------------------------
                // Verificar vacío
                // --------------------------------------------
                if let Some(children) = directories.get(&entry.ino) {
                    if children.len() > 2 {
                        log_point!(format!(
                            "rmdir(): directory {} NOT EMPTY ({} entries)",
                            entry.ino,
                            children.len()
                        ));
                        exit_code = Some(libc::ENOTEMPTY);
                    }
                }

                // Si NO se ha puesto error → borrar
                if exit_code.is_none() {
                    log_point!(format!("rmdir(): removing inode {}", entry.ino));

                    // Quitar del padre
                    if let Some(parent_entries) = directories.get_mut(&parent) {
                        parent_entries.retain(|e| e.ino != entry.ino);
                    }

                    directories.remove(&entry.ino);
                    inodes.remove(&entry.ino);

                    // Reducir nlink del padre
                    if let Some(parent_inode) = inodes.get_mut(&parent) {
                        parent_inode.nlink -= 1;
                    }
                }
            }

            // Los locks se sueltan automáticamente aquí
        }

        // --------------------------------------------
        // REPLY FINAL
        // --------------------------------------------
        match exit_code {
            None => {
                // Directory tree cambió → metadata sucia
                self.mark_dirty();

                reply.ok();
                log_exit!("rmdir() -> EXIT OK");
            }
            Some(errno) => {
                reply.error(errno);
                log_exit!(format!("rmdir() -> EXIT ERR {}", errno));
            }
        }
    }

    fn rename(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        newparent: u64,
        newname: &std::ffi::OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let name = name.to_string_lossy().to_string();
        let newname = newname.to_string_lossy().to_string();

        log_point!(format!(
            "ENTER rename(): parent={}, name='{}', newparent={}, newname='{}'",
            parent, name, newparent, newname
        ));

        let mut exit_code: Option<i32> = None; // None = OK; Some(errno) = error

        {
            log_point!("rename() -> locking directories");
            let mut directories = self.directories.lock().unwrap();
            log_point!("rename() -> locks acquired");

            // ----------------------------------------------------------
            // Buscar entrada en el parent original
            // ----------------------------------------------------------
            let entry_info = directories
                .get_mut(&parent)
                .and_then(|entries| {
                    entries
                        .iter()
                        .position(|e| e.name == name)
                        .map(|pos| (pos, entries))
                });

            if entry_info.is_none() {
                log_point!(format!(
                    "rename(): entry '{}' not found in parent {}",
                    name, parent
                ));
                exit_code = Some(libc::ENOENT);
            } else {
                let (pos, parent_entries) = entry_info.unwrap();
                log_point!(format!(
                    "rename(): found '{}' at pos {} in parent {}",
                    name, pos, parent
                ));

                // ----------------------------------------------------------
                // Quitar la entrada del directorio original
                // ----------------------------------------------------------
                let mut entry = parent_entries.remove(pos);
                log_point!(format!(
                    "rename(): removed old entry '{}' (ino={}) from parent {}",
                    name, entry.ino, parent
                ));

                // ----------------------------------------------------------
                // Actualizar nombre
                // ----------------------------------------------------------
                entry.name = newname.clone();
                log_point!(format!(
                    "rename(): updated name '{}' -> '{}'",
                    name, newname
                ));

                // ----------------------------------------------------------
                // Insertar en el nuevo parent
                // ----------------------------------------------------------
                directories
                    .entry(newparent)
                    .or_insert_with(Vec::new)
                    .push(entry);

                log_point!(format!(
                    "rename(): inserted updated entry into newparent {}",
                    newparent
                ));
            }

            // Locks salen aquí
        }

        // ----------------------------------------------------------
        // REPLY FINAL
        // ----------------------------------------------------------
        match exit_code {
            None => {
                // Directory tree cambió → metadata sucia
                self.mark_dirty();

                reply.ok();
                log_exit!("rename() -> EXIT OK");
            }
            Some(errno) => {
                reply.error(errno);
                log_exit!(format!("rename() -> EXIT ERR {}", errno));
            }
        }
    }

    fn flush(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty,
    ) {
        log_point!(format!("ENTER flush(): ino={}, fh={}", ino, fh));

        // Nota: flush no escribe metadata, solo notifica el cierre del descriptor.
        // Usamos release() para decidir cuándo sincronizar metadata.
        reply.ok();

        log_exit!(format!("flush(): completed for ino={}, fh={}", ino, fh));
    }

    fn fsync(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        datasync: bool,
        reply: ReplyEmpty,
    ) {
        log_point!(format!(
            "ENTER fsync(): ino={}, fh={}, datasync={}",
            ino, fh, datasync
        ));

        match self.sync_if_dirty() {
            Ok(_) => {
                log_point!("fsync(): sync_if_dirty() completed OK");
                reply.ok();
            }
            Err(e) => {
                log_point!(format!("ERROR in fsync(): sync_if_dirty() failed -> {}", e));
                reply.error(libc::EIO);
            }
        }

        log_exit!(format!("EXIT fsync(): ino={}, fh={}", ino, fh));
    }

    fn access(&mut self, _req: &Request, ino: u64, mask: i32, reply: ReplyEmpty) {
        log_point!(format!("ENTER access(): ino={}, mask={}", ino, mask));

        let inodes = self.inodes.lock().unwrap();

        if inodes.contains_key(&ino) {
            log_point!(format!("access(): inode {} EXISTS -> granting access", ino));
            reply.ok();
        } else {
            log_point!(format!("access(): inode {} NOT FOUND -> ENOENT", ino));
            reply.error(libc::ENOENT);
        }

        log_exit!(format!("EXIT access(): ino={}", ino));
    }

    fn statfs(&mut self, _req: &Request, ino: u64, reply: ReplyStatfs) {
        log_point!(format!("ENTER statfs(): ino={}", ino));

        let block_bitmap = self.block_bitmap.lock().unwrap();
        let _inode_bitmap = self.inode_bitmap.lock().unwrap();

        let block_size = self.storage.lock().unwrap().bytes_per_block() as u32;
        let total_blocks = self.config.total_blocks as u64;

        // Count free blocks
        let mut free_blocks = 0u64;
        for i in 0..self.config.total_blocks as usize {
            if !block_bitmap.is_set(i) {
                free_blocks += 1;
            }
        }

        let used_inodes = self.inodes.lock().unwrap().len() as u64;
        let free_inodes = self.config.total_inodes as u64 - used_inodes;

        log_point!(format!(
            "statfs(): block_size={}, total_blocks={}, free_blocks={}, used_inodes={}, free_inodes={}",
            block_size, total_blocks, free_blocks, used_inodes, free_inodes
        ));

        reply.statfs(
            total_blocks,                         // blocks
            free_blocks,                          // bfree
            free_blocks,                          // bavail
            self.config.total_inodes as u64,      // files
            free_inodes,                          // ffree
            block_size,                           // bsize
            255,                                  // namelen
            block_size,                           // frsize
        );

        log_exit!(format!("EXIT statfs(): ino={}", ino));
    }

    fn opendir(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        log_point!(format!("ENTER opendir(): ino={}, flags={}", ino, flags));

        let inodes = self.inodes.lock().unwrap();

        if let Some(inode) = inodes.get(&ino) {
            if inode.is_dir() {
                let fh = self.allocate_fh();
                log_point!(format!(
                    "opendir(): allocated fh={} for dir inode {}",
                    fh, ino
                ));
                reply.opened(fh, 0);
            } else {
                log_point!(format!("opendir(): inode {} is NOT a directory", ino));
                reply.error(libc::ENOTDIR);
            }
        } else {
            log_point!(format!("opendir(): inode {} NOT FOUND", ino));
            reply.error(libc::ENOENT);
        }

        log_exit!(format!("EXIT opendir(): ino={}", ino));
    }

    fn release(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        log_point!(format!("ENTER release(): ino={}, fh={}", ino, fh));

        // Primero intentamos sincronizar metadata si está sucia.
        if let Err(e) = self.sync_if_dirty() {
            log_point!(format!("release(): ERROR syncing metadata -> {}", e));
            reply.error(libc::EIO);
            log_exit!(format!("EXIT release(): ino={}, fh={} (ERROR)", ino, fh));
            return;
        }

        let mut open_files = self.open_files.lock().unwrap();
        if open_files.remove(&fh).is_some() {
            log_point!(format!(
                "release(): removed fh={} mapped to ino={}",
                fh, ino
            ));
        } else {
            log_point!(format!("release(): fh={} not found in open_files", fh));
        }

        reply.ok();
        log_exit!(format!("EXIT release(): ino={}, fh={}", ino, fh));
    }

    fn releasedir(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        _flags: i32,
        reply: ReplyEmpty,
    ) {
        log_point!(format!("ENTER releasedir(): ino={}, fh={}", ino, fh));

        // También aquí sincronizamos si hay metadata sucia, para cubrir cambios
        // que sólo afecten directorios (mkdir/rename/rmdir, etc.).
        if let Err(e) = self.sync_if_dirty() {
            log_point!(format!("releasedir(): ERROR syncing metadata -> {}", e));
            reply.error(libc::EIO);
            log_exit!(format!("EXIT releasedir(): ino={}, fh={} (ERROR)", ino, fh));
            return;
        }

        let mut open_files = self.open_files.lock().unwrap();
        if open_files.remove(&fh).is_some() {
            log_point!(format!(
                "releasedir(): removed fh={} for directory ino={}",
                fh, ino
            ));
        } else {
            log_point!(format!("releasedir(): fh={} not found in open_files", fh));
        }

        reply.ok();
        log_exit!(format!("EXIT releasedir(): ino={}, fh={}", ino, fh));
    }
}