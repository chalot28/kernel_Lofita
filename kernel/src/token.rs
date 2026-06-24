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
    pub vmas:          Vec<Vma>,
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
}
