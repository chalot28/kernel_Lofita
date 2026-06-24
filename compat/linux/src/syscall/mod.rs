// compat/linux/src/syscall/mod.rs
// Written in Rust
// Syscall routing for compatibility subsystem.

pub mod memory;
pub mod fs;
pub mod net;
pub mod process;

use memory::MemorySyscalls;
use fs::FsSyscalls;
use net::NetSyscalls;
use process::ProcessSyscalls;

pub struct SyscallRouter {
    pub memory: MemorySyscalls,
}

impl SyscallRouter {
    pub fn new() -> Self {
        SyscallRouter {
            memory: MemorySyscalls::new(),
        }
    }

    pub fn route(
        &mut self,
        syscall_num: u64,
        args: [u64; 6],
        session_id: u64,
        token_id: u64,
        process_id: u32,
    ) -> i64 {
        match syscall_num {
            1 => {
                // sys_write(fd, buf_ptr, count)
                FsSyscalls::sys_write(session_id, args[0], args[1], args[2])
            }
            2 => {
                // sys_open(path_ptr, path_len, flags)
                let path_ptr = args[0] as *const u8;
                let path_len = args[1] as usize;
                let is_write = args[2] != 0;
                let path_slice = unsafe { std::slice::from_raw_parts(path_ptr, path_len) };
                let path = std::str::from_utf8(path_slice).unwrap_or("unknown");
                FsSyscalls::sys_open(session_id, path, is_write)
            }
            3 => {
                // sys_close(fd)
                FsSyscalls::sys_close(session_id, args[0])
            }
            9 => {
                // sys_mmap(length)
                self.memory.sys_mmap(token_id, args[0])
            }
            12 => {
                // sys_brk(new_brk)
                self.memory.sys_brk(token_id, args[0])
            }
            41 => {
                // sys_socket
                NetSyscalls::sys_socket(process_id, true)
            }
            60 => {
                // sys_exit
                ProcessSyscalls::sys_exit(process_id, args[0] as i64)
            }
            _ => -38, // -ENOSYS
        }
    }
}
