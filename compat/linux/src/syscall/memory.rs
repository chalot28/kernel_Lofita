// compat/linux/src/syscall/memory.rs
// Written in Rust
// Translation of Linux memory management syscalls.

extern "C" {
    pub fn rust_allocate_memory(token_id: u64, size: usize, is_writeable: bool, is_executable: bool) -> usize;
}

pub struct MemorySyscalls {
    pub heap_brk: u64,
    pub heap_limit: u64,
}

impl MemorySyscalls {
    pub fn new() -> Self {
        MemorySyscalls {
            heap_brk: 0x40000000,
            heap_limit: 0x40000000,
        }
    }

    pub fn sys_brk(&mut self, token_id: u64, new_brk: u64) -> i64 {
        if new_brk == 0 {
            return self.heap_brk as i64;
        }
        if new_brk < self.heap_brk {
            return -22; // -EINVAL
        }
        if new_brk > self.heap_limit {
            let needed = (new_brk - self.heap_limit) as usize;
            let allocated = unsafe {
                rust_allocate_memory(token_id, needed, true, false)
            };
            if allocated == 0 {
                return -12; // -ENOMEM
            }
            self.heap_limit = (allocated + needed) as u64;
        }
        self.heap_brk = new_brk;
        new_brk as i64
    }

    pub fn sys_mmap(&mut self, token_id: u64, length: u64) -> i64 {
        let allocated = unsafe {
            rust_allocate_memory(token_id, length as usize, true, false)
        };
        if allocated == 0 {
            -12 // -ENOMEM
        } else {
            allocated as i64
        }
    }
}
