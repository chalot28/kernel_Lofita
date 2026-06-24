// kernel/src/vfs.rs
// Written in Rust
// Virtual File System (VFS) and file descriptor tables.

use std::collections::HashMap;

pub struct FileDescriptor {
    pub fd: u32,
    pub path: String,
    pub is_writeable: bool,
}

pub struct VfsState {
    pub fd_tables: HashMap<u64, HashMap<u32, FileDescriptor>>, // session_id -> { fd -> descriptor }
    pub next_fd: u32,
}

impl VfsState {
    pub fn new() -> Self {
        VfsState {
            fd_tables: HashMap::new(),
            next_fd: 3, // 0: stdin, 1: stdout, 2: stderr
        }
    }

    pub fn open(&mut self, session_id: u64, path: &str, is_write: bool) -> Result<u32, &'static str> {
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
}
