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
        process_id: u32,
        capabilities_mask: u32,
    ) -> i64 {
        let has_mem_alloc = (capabilities_mask & 1) != 0;
        let has_fs_read = (capabilities_mask & 4) != 0;
        let has_fs_write = (capabilities_mask & 8) != 0;
        let has_net_connect = (capabilities_mask & 16) != 0;

        match syscall_num {
            1 => FsSyscalls::sys_write(args[0], args[1], args[2], has_fs_write),
            2 => FsSyscalls::sys_open(has_fs_read),
            9 => {
                if !has_mem_alloc {
                    -12 // -ENOMEM
                } else {
                    0x30000000
                }
            }
            12 => self.memory.sys_brk(args[0], has_mem_alloc),
            41 => NetSyscalls::sys_socket(process_id, has_net_connect),
            60 => ProcessSyscalls::sys_exit(process_id, args[0] as i64),
            _ => -38, // -ENOSYS
        }
    }
}
