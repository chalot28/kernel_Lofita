// compat/linux/src/syscall/memory.rs
// Written in Rust
// Translation of Linux memory management syscalls.

pub struct MemorySyscalls {
    pub heap_brk: u64,
}

impl MemorySyscalls {
    pub fn new() -> Self {
        MemorySyscalls { heap_brk: 0x40000000 }
    }

    pub fn sys_brk(&mut self, new_brk: u64, has_mem_alloc: bool) -> i64 {
        if new_brk == 0 {
            return self.heap_brk as i64;
        }
        if !has_mem_alloc {
            return -12; // -ENOMEM
        }
        if new_brk >= self.heap_brk {
            self.heap_brk = new_brk;
            new_brk as i64
        } else {
            -22 // -EINVAL
        }
    }
}
