// kernel/src/session.rs
// Written in Rust
// Session structures mapping active processes.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use crate::token::Token;

pub struct Session {
    pub id: u64,
    pub token: Arc<Mutex<Token>>,
    pub expiry: Instant,
    pub is_active: bool,
}

impl Session {
    pub fn is_valid(&self, now: Instant) -> bool {
        if !self.is_active {
            return false;
        }
        if now > self.expiry {
            return false;
        }
        let token = self.token.lock().unwrap();
        !token.is_expired(now)
    }
}
