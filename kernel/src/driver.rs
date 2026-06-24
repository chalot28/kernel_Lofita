// kernel/src/driver.rs
// Written in Rust (no_std)
// Monolithic Char Device Driver manager: /dev/null, /dev/urandom, /dev/fb0.
//
// no_std changes:
//   - std::collections::HashMap   → hashbrown::HashMap
//   - std::sync::Mutex            → spin::Mutex
//   - Vec (heap)                  → alloc::vec::Vec
//   - String                      → alloc::string::String
//   - std::cmp::min               → core::cmp::min
//   - Box<dyn Trait>              → requires alloc (provided by global allocator)
//   - println!                    → kprint!
//
// Note: dyn CharDriver in a Box requires the alloc crate.
// Dynamic dispatch (vtables) works fine in no_std with alloc.

use hashbrown::HashMap;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;
use crate::kprint;

// ---------------------------------------------------------------------------
// CharDriver trait
// ---------------------------------------------------------------------------

pub trait CharDriver: Send + Sync {
    fn open(&self)                     -> Result<(), &'static str>;
    fn read(&self, size: usize)        -> Vec<u8>;
    fn write(&self, data: &[u8])       -> usize;
}

// ---------------------------------------------------------------------------
// /dev/null
// ---------------------------------------------------------------------------

pub struct NullDriver;

impl CharDriver for NullDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }
    fn read(&self, _size: usize) -> Vec<u8> { Vec::new() }    // EOF
    fn write(&self, data: &[u8]) -> usize { data.len() }       // Discard
}

// ---------------------------------------------------------------------------
// /dev/urandom  — deterministic pseudo-random bytes (no entropy source yet)
// ---------------------------------------------------------------------------

pub struct UrandomDriver;

impl CharDriver for UrandomDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }

    fn read(&self, size: usize) -> Vec<u8> {
        // LCG-based PRNG seeded with size (no entropy on bare metal yet)
        let mut bytes = Vec::with_capacity(size);
        let mut state: u64 = size as u64 ^ 0xDEADBEEF_CAFEBABE;
        for _ in 0..size {
            state = state.wrapping_mul(6364136223846793005)
                         .wrapping_add(1442695040888963407);
            bytes.push((state >> 33) as u8);
        }
        bytes
    }

    fn write(&self, data: &[u8]) -> usize { data.len() } // Discard writes
}

// ---------------------------------------------------------------------------
// /dev/fb0  — VGA framebuffer stub (4KB pixel buffer in a spinlock)
// ---------------------------------------------------------------------------

pub struct Fb0Driver {
    pub buffer: Mutex<Vec<u8>>,
}

impl CharDriver for Fb0Driver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }

    fn read(&self, size: usize) -> Vec<u8> {
        let buf   = self.buffer.lock();
        let limit = core::cmp::min(size, buf.len());
        buf[0..limit].to_vec()
    }

    fn write(&self, data: &[u8]) -> usize {
        let mut buf   = self.buffer.lock();
        let limit = core::cmp::min(data.len(), buf.len());
        buf[0..limit].copy_from_slice(&data[0..limit]);
        kprint!("[Driver fb0] Framebuffer updated: {} bytes.\n", limit);
        limit
    }
}

// ---------------------------------------------------------------------------
// DriverManager
// ---------------------------------------------------------------------------

pub struct DriverManager {
    pub drivers: HashMap<String, Box<dyn CharDriver>>,
}

impl DriverManager {
    pub fn new() -> Self {
        let mut dm = DriverManager {
            drivers: HashMap::new(),
        };

        dm.drivers.insert("/dev/null".to_string(),    Box::new(NullDriver));
        dm.drivers.insert("/dev/urandom".to_string(), Box::new(UrandomDriver));
        dm.drivers.insert("/dev/fb0".to_string(),     Box::new(Fb0Driver {
            buffer: Mutex::new(vec![0u8; 4096]),
        }));

        kprint!("[DriverManager] Registered: /dev/null, /dev/urandom, /dev/fb0\n");
        dm
    }

    pub fn is_device(&self, path: &str) -> bool {
        self.drivers.contains_key(path)
    }

    pub fn read_device(&self, path: &str, size: usize) -> Option<Vec<u8>> {
        self.drivers.get(path).map(|drv| drv.read(size))
    }

    pub fn write_device(&self, path: &str, data: &[u8]) -> Option<usize> {
        self.drivers.get(path).map(|drv| drv.write(data))
    }
}
