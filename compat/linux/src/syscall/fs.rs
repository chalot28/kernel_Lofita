// compat/linux/src/syscall/fs.rs
// Written in Rust
// Translation of Linux file system syscalls.

pub struct FsSyscalls;

impl FsSyscalls {
    pub fn sys_write(fd: u64, buf_ptr: u64, count: u64, has_fs_write: bool) -> i64 {
        if !has_fs_write {
            return -13; // -EACCES
        }
        if fd == 1 || fd == 2 {
            println!("[compat/linux/fs] stdout: wrote {} bytes from buffer 0x{:x}", count, buf_ptr);
            count as i64
        } else {
            count as i64
        }
    }

    pub fn sys_open(has_fs_read: bool) -> i64 {
        if !has_fs_read {
            return -13; // -EACCES
        }
        100 // Simulated FD
    }
}
