// compat/linux/src/syscall/fs.rs
// Written in Rust
// Translation of Linux file system syscalls.

extern "C" {
    pub fn rust_vfs_open(session_id: u64, path_ptr: *const u8, path_len: usize, write_val: u32) -> u32;
    pub fn rust_vfs_close(session_id: u64, fd: u32) -> i32;
    pub fn rust_vfs_read(session_id: u64, fd: u32, buf_ptr: *mut u8, buf_len: usize) -> i32;
    pub fn rust_vfs_write(session_id: u64, fd: u32, data_ptr: *const u8, data_len: usize) -> i32;
}

pub struct FsSyscalls;

impl FsSyscalls {
    pub fn sys_open(session_id: u64, path: &str, is_write: bool) -> i64 {
        let path_bytes = path.as_bytes();
        let write_val = if is_write { 1 } else { 0 };
        let fd = unsafe {
            rust_vfs_open(session_id, path_bytes.as_ptr(), path_bytes.len(), write_val)
        };
        if fd == 0 {
            -2 // -ENOENT
        } else {
            fd as i64
        }
    }

    pub fn sys_write(session_id: u64, fd: u64, buf_ptr: u64, count: u64) -> i64 {
        let result = unsafe {
            rust_vfs_write(session_id, fd as u32, buf_ptr as *const u8, count as usize)
        };
        if result < 0 {
            -9 // -EBADF / -EACCES
        } else {
            result as i64
        }
    }

    pub fn sys_read(session_id: u64, fd: u64, buf_ptr: u64, count: u64) -> i64 {
        let result = unsafe {
            rust_vfs_read(session_id, fd as u32, buf_ptr as *mut u8, count as usize)
        };
        if result < 0 {
            -9 // -EBADF / -EACCES
        } else {
            result as i64
        }
    }

    pub fn sys_close(session_id: u64, fd: u64) -> i64 {
        let result = unsafe {
            rust_vfs_close(session_id, fd as u32)
        };
        result as i64
    }
}
