// kernel/src/session.rs
// Written in Rust (no_std)
// Session binds a Token to an active time window (expiry_tick instead of Instant).

use alloc::sync::Arc;
use spin::Mutex;
use crate::token::Token;

pub struct Session {
    pub id:        u64,
    pub token:     Arc<Mutex<Token>>,
    pub expiry_tick: u64,   // Kernel tick count at expiry (replaces std::time::Instant)
    pub is_active: bool,
}
