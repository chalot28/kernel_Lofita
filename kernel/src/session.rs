// kernel/src/session.rs
// Written in Rust (no_std)
// Session binds a Token to an active time window (expiry_tick instead of Instant).

use alloc::sync::Arc;
use spin::Mutex;
use crate::token::Token;

extern "C" {
    fn switch_page_directory(phys_addr: usize);
}

pub struct Session {
    pub id:        u64,
    pub token:     Arc<Mutex<Token>>,
    pub expiry_tick: u64,   // Kernel tick count at expiry (replaces std::time::Instant)
    pub is_active: bool,
}

impl Session {
    pub fn activate(&mut self) {
        self.is_active = true;
        let token = self.token.lock();
        if token.pml4_phys_addr != 0 {
            unsafe { switch_page_directory(token.pml4_phys_addr); }
        }
    }
}
