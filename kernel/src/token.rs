// kernel/src/token.rs
// Written in Rust
// Token structures and TMD (Token Memory Descriptor) logic.

use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use crate::capability::{Capability, PrivilegeLevel};
use crate::vasm::Vma;

pub struct Token {
    pub id: u64,
    pub name: String,
    pub privilege: PrivilegeLevel,
    pub parent: Option<Weak<Mutex<Token>>>,
    pub children: Vec<Arc<Mutex<Token>>>,
    pub vmas: Vec<Vma>,
    pub memory_limit: usize,
    pub memory_used: usize,
    pub is_permanent: bool,
    pub expiry: Instant,
    pub capabilities: Capability,
    pub run_count: u32,
    pub is_deprecated: bool,
}

impl Token {
    pub fn is_expired(&self, now: Instant) -> bool {
        if self.is_permanent {
            false
        } else {
            now > self.expiry
        }
    }
}
