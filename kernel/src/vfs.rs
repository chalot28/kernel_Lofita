// kernel/src/vfs.rs
// Written in Rust (no_std)
// Virtual File System (VFS), RAMFS, and file descriptor tables.
//
// no_std changes:
//   - std::collections::HashMap → hashbrown::HashMap
//   - std::str::from_utf8       → core::str::from_utf8
//   - std::cmp::min             → core::cmp::min
//   - println!                  → kprint!
//   - String, Vec               → alloc::string::String, alloc::vec::Vec

use hashbrown::HashMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str;
use crate::kprint;

pub struct FileDescriptor {
    pub fd:           u32,
    pub path:         String,
    pub is_writeable: bool,
    pub offset:       usize,
}

pub struct RamFile {
    pub path:    String,
    pub content: Vec<u8>,
}

pub struct VfsState {
    /// session_id → { fd → descriptor }
    pub fd_tables: HashMap<u64, HashMap<u32, FileDescriptor>>,
    /// path → file
    pub ramfs:     HashMap<String, RamFile>,
    pub next_fd:   u32,
}

impl VfsState {
    pub fn new() -> Self {
        VfsState {
            fd_tables: HashMap::new(),
            ramfs:     HashMap::new(),
            next_fd:   3, // 0: stdin, 1: stdout, 2: stderr
        }
    }

    pub fn populate_from_initramfs(&mut self, decompressed_data: &[u8]) {
        let text = str::from_utf8(decompressed_data).unwrap_or("");
        let mut current_path    = String::new();
        let mut current_content = String::new();

        for line in text.lines() {
            if line.starts_with("FILE:") {
                if !current_path.is_empty() {
                    self.ramfs.insert(current_path.clone(), RamFile {
                        path:    current_path.clone(),
                        content: current_content.as_bytes().to_vec(),
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
                path:    current_path,
                content: current_content.as_bytes().to_vec(),
            });
        }
        kprint!("[VFS] Loaded {} files from initramfs.\n", self.ramfs.len());
    }

    pub fn open(&mut self, session_id: u64, path: &str, is_write: bool) -> Result<u32, &'static str> {
        // Path traversal guard
        if path.contains("..") || path.contains('\0') {
            return Err("Invalid path");
        }

        if is_write && !path.starts_with("/dev/") && !self.ramfs.contains_key(path) {
            self.ramfs.insert(path.to_string(), RamFile {
                path:    path.to_string(),
                content: Vec::new(),
            });
            kprint!("[VFS] Created RAMFS file '{}'\n", path);
        }

        if !path.starts_with("/dev/") && !self.ramfs.contains_key(path) {
            return Err("File not found");
        }

        let table = self.fd_tables.entry(session_id).or_insert_with(HashMap::new);
        let fd    = self.next_fd;
        self.next_fd += 1;

        table.insert(fd, FileDescriptor {
            fd,
            path: path.to_string(),
            is_writeable: is_write,
            offset: 0,
        });

        kprint!("[VFS] Session {} opened '{}' -> FD {}\n", session_id, path, fd);
        Ok(fd)
    }

    pub fn close(&mut self, session_id: u64, fd: u32) -> Result<(), &'static str> {
        if let Some(table) = self.fd_tables.get_mut(&session_id) {
            if table.remove(&fd).is_some() {
                kprint!("[VFS] Session {} closed FD {}\n", session_id, fd);
                return Ok(());
            }
        }
        Err("Invalid file descriptor")
    }

    pub fn check_fd_permission(&self, session_id: u64, fd: u32, require_write: bool) -> bool {
        if let Some(table) = self.fd_tables.get(&session_id) {
            if let Some(desc) = table.get(&fd) {
                return !(require_write && !desc.is_writeable);
            }
        }
        false
    }

    pub fn read(
        &mut self,
        session_id: u64,
        fd: u32,
        size: usize,
        dm: &crate::driver::DriverManager,
    ) -> Result<Vec<u8>, &'static str> {
        let table = self.fd_tables.get_mut(&session_id).ok_or("Session not found")?;
        let desc  = table.get_mut(&fd).ok_or("Invalid file descriptor")?;

        if desc.path.starts_with("/dev/") {
            if let Some(data) = dm.read_device(&desc.path, desc.offset, size) {
                desc.offset += data.len();
                return Ok(data);
            }
        }

        if let Some(file) = self.ramfs.get(&desc.path) {
            if desc.offset >= file.content.len() {
                return Ok(Vec::new()); // EOF
            }
            let limit = core::cmp::min(desc.offset + size, file.content.len());
            let data = file.content[desc.offset..limit].to_vec();
            desc.offset += data.len();
            return Ok(data);
        }
        Err("File not found in RAMFS")
    }

    pub fn write(
        &mut self,
        session_id: u64,
        fd: u32,
        data: &[u8],
        dm: &crate::driver::DriverManager,
    ) -> Result<usize, &'static str> {
        let (path, offset) = {
            let table = self.fd_tables.get(&session_id).ok_or("Session not found")?;
            let desc  = table.get(&fd).ok_or("Invalid file descriptor")?;
            if !desc.is_writeable { return Err("FD not writable"); }
            (desc.path.clone(), desc.offset)
        };

        let written = if path.starts_with("/dev/") {
            dm.write_device(&path, offset, data).ok_or("Device write failed")?
        } else if let Some(file) = self.ramfs.get_mut(&path) {
            let end = offset + data.len();
            if end > file.content.len() {
                file.content.resize(end, 0);
            }
            file.content[offset..end].copy_from_slice(data);
            kprint!("[VFS] Wrote {} bytes to '{}' at offset {}\n", data.len(), path, offset);
            data.len()
        } else {
            return Err("File not found in RAMFS");
        };

        // Update offset
        let table = self.fd_tables.get_mut(&session_id).unwrap();
        let desc  = table.get_mut(&fd).unwrap();
        desc.offset += written;
        Ok(written)
    }
}
