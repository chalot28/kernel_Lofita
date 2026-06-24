// compat/linux/src/syscall/process.rs
// Written in Rust
// Translation of Linux thread/process control syscalls.

pub struct ProcessSyscalls;

impl ProcessSyscalls {
    pub fn sys_exit(process_id: u32, code: i64) -> i64 {
        println!("[compat/linux/process] Process {} exited with code {}", process_id, code);
        0
    }
}
