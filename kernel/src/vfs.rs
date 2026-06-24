// kernel/src/vfs.rs
// Written in Rust
// Virtual File System (VFS), RAMFS, and file descriptor tables.

use std::collections::HashMap;

pub struct FileDescriptor {
    pub fd: u32,
    pub path: String,
    pub is_writeable: bool,
}

pub struct RamFile {
    pub path: String,
    pub content: Vec<u8>,
}

pub struct VfsState {
    pub fd_tables: HashMap<u64, HashMap<u32, FileDescriptor>>, // session_id -> { fd -> descriptor }
    pub ramfs: HashMap<String, RamFile>,                       // path -> file
    pub next_fd: u32,
}

impl VfsState {
    pub fn new() -> Self {
        VfsState {
            fd_tables: HashMap::new(),
            ramfs: HashMap::new(),
            next_fd: 3, // 0: stdin, 1: stdout, 2: stderr
        }
    }

    pub fn populate_from_initramfs(&mut self, decompressed_data: &[u8]) {
        let text = std::str::from_utf8(decompressed_data).unwrap_or("");
        let mut current_path = String::new();
        let mut current_content = String::new();
        
        for line in text.lines() {
            if line.starts_with("FILE:") {
                if !current_path.is_empty() {
                    self.ramfs.insert(current_path.clone(), RamFile {
                        path: current_path.clone(),
                        content: current_content.clone().into_bytes(),
                    });
                }
                current_path = line["FILE:".len()..].trim().to_string();
                current_content.clear();
            } else {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(line);
            }
        }
        if !current_path.is_empty() {
            self.ramfs.insert(current_path.clone(), RamFile {
                path: current_path,
                content: current_content.into_bytes(),
            });
        }
        println!("[VFS] Loaded {} files from decompressed initramfs.", self.ramfs.len());
    }

    pub fn open(&mut self, session_id: u64, path: &str, is_write: bool) -> Result<u32, &'static str> {
        // If write is requested and path doesn't start with /dev/ and file doesn't exist, we create it in RAMFS
        if is_write && !path.starts_with("/dev/") && !self.ramfs.contains_key(path) {
            self.ramfs.insert(path.to_string(), RamFile {
                path: path.to_string(),
                content: Vec::new(),
            });
            println!("[VFS] Created new RAMFS file '{}'", path);
        }

        // Validate file exists in ramfs or is device
        if !path.starts_with("/dev/") && !self.ramfs.contains_key(path) {
            return Err("File not found");
        }

        let table = self.fd_tables.entry(session_id).or_insert_with(HashMap::new);
        let fd = self.next_fd;
        self.next_fd += 1;

        table.insert(fd, FileDescriptor {
            fd,
            path: path.to_string(),
            is_writeable: is_write,
        });

        println!("[VFS] Session {} opened file '{}' -> assigned FD {}", session_id, path, fd);
        Ok(fd)
    }

    pub fn close(&mut self, session_id: u64, fd: u32) -> Result<(), &'static str> {
        if let Some(table) = self.fd_tables.get_mut(&session_id) {
            if table.remove(&fd).is_some() {
                println!("[VFS] Session {} closed FD {}", session_id, fd);
                return Ok(());
            }
        }
        Err("Invalid file descriptor")
    }

    pub fn check_fd_permission(&self, session_id: u64, fd: u32, require_write: bool) -> bool {
        if let Some(table) = self.fd_tables.get(&session_id) {
            if let Some(desc) = table.get(&fd) {
                if require_write && !desc.is_writeable {
                    return false;
                }
                return true;
            }
        }
        false
    }

    pub fn read(&self, session_id: u64, fd: u32, size: usize, dm: &crate::driver::DriverManager) -> Result<Vec<u8>, &'static str> {
        let table = self.fd_tables.get(&session_id).ok_or("Session not found")?;
        let desc = table.get(&fd).ok_or("Invalid file descriptor")?;

        if desc.path.starts_with("/dev/") {
            if let Some(data) = dm.read_device(&desc.path, size) {
                return Ok(data);
            }
        }
        
        // RAMFS read
        if let Some(file) = self.ramfs.get(&desc.path) {
            let limit = std::cmp::min(size, file.content.len());
            return Ok(file.content[0..limit].to_vec());
        }
        
        Err("File not found in RAMFS")
    }

    pub fn write(&mut self, session_id: u64, fd: u32, data: &[u8], dm: &crate::driver::DriverManager) -> Result<usize, &'static str> {
        let table = self.fd_tables.get(&session_id).ok_or("Session not found")?;
        let desc = table.get(&fd).ok_or("Invalid file descriptor")?;

        if !desc.is_writeable {
            return Err("File descriptor is not writeable");
        }

        if desc.path.starts_with("/dev/") {
            if let Some(bytes_written) = dm.write_device(&desc.path, data) {
                return Ok(bytes_written);
            }
        }

        // RAMFS write
        if let Some(file) = self.ramfs.get_mut(&desc.path) {
            file.content = data.to_vec();
            println!("[VFS] Wrote {} bytes to RAMFS file '{}'", data.len(), desc.path);
            return Ok(data.len());
        }

        Err("File not found in RAMFS")
    }
}
