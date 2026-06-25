// kernel/src/token.rs
// Written in Rust (no_std)
// Token structures and TMD (Token Memory Descriptor) logic.
//
// Key no_std changes:
//   - std::time::Instant → kernel tick counter (u64 from global KERNEL_TICKS)
//   - std::sync::Arc/Mutex → alloc::sync::Arc + spin::Mutex
//   - std::sync::Weak → alloc::sync::Weak

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use spin::Mutex;

use crate::capability::{Capability, PrivilegeLevel};
use crate::vasm::Vma;

pub struct Token {
    pub id:            u64,
    pub name:          String,
    pub privilege:     PrivilegeLevel,
    pub parent:        Option<Weak<Mutex<Token>>>,
    pub children:      Vec<Arc<Mutex<Token>>>,
    pub vmas:          BTreeMap<usize, Vma>,
    pub pml4_phys_addr: usize,
    pub memory_limit:  usize,
    pub memory_used:   usize,
    pub is_permanent:  bool,
    /// Expiry expressed as a kernel tick count instead of wall-clock Instant.
    /// KERNEL_TICKS is a global u64 incremented on every timer interrupt.
    pub expiry_tick:   u64,
    pub capabilities:  Capability,
    pub run_count:     u32,
    pub is_deprecated: bool,
}

impl Token {
    /// Returns true if the token has expired relative to the current tick count.
    pub fn is_expired(&self, current_tick: u64) -> bool {
        if self.is_permanent {
            false
        } else {
            current_tick > self.expiry_tick
        }
    }

    /// Checks if adding `size` bytes would exceed the token's memory limit.
    pub fn check_quota(&self, size: usize) -> bool {
        self.memory_used.saturating_add(size) <= self.memory_limit
    }

    /// Cleans up all memory associated with this token.
    /// Should be called when the token expires or is explicitly killed.
    pub fn cleanup(&mut self) {
        for (_, vma) in self.vmas.iter() {
            if !vma.phys_ptr.is_null() && vma.size > 0 {
                let pages = (vma.size + 4095) / 4096;
                unsafe { crate::vasm::phys_free(vma.phys_ptr, pages); }
                // Also unmap the virtual address
                unsafe { crate::vasm::page_table_unmap(vma.start_addr); }
            }
        }
        self.vmas.clear();
        self.memory_used = 0;
    }
}
