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
    fn open(&self)                                  -> Result<(), &'static str>;
    fn read(&self, offset: usize, size: usize)      -> Vec<u8>;
    fn write(&self, offset: usize, data: &[u8])     -> usize;
}

// ---------------------------------------------------------------------------
// /dev/null
// ---------------------------------------------------------------------------

pub struct NullDriver;

impl CharDriver for NullDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }
    fn read(&self, _offset: usize, _size: usize) -> Vec<u8> { Vec::new() }    // EOF
    fn write(&self, _offset: usize, data: &[u8]) -> usize { data.len() }       // Discard
}

// ---------------------------------------------------------------------------
// /dev/urandom  — deterministic pseudo-random bytes (no entropy source yet)
// ---------------------------------------------------------------------------

pub struct UrandomDriver;

impl CharDriver for UrandomDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }

    fn read(&self, _offset: usize, size: usize) -> Vec<u8> {
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

    fn write(&self, _offset: usize, data: &[u8]) -> usize { data.len() } // Discard writes
}

// ---------------------------------------------------------------------------
// /dev/fb0  — VGA framebuffer stub (4KB pixel buffer in a spinlock)
// ---------------------------------------------------------------------------

pub struct Fb0Driver {
    pub buffer: Mutex<Vec<u8>>,
}

impl CharDriver for Fb0Driver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }

    fn read(&self, offset: usize, size: usize) -> Vec<u8> {
        let buf   = self.buffer.lock();
        if offset >= buf.len() { return Vec::new(); }
        let limit = core::cmp::min(offset + size, buf.len());
        buf[offset..limit].to_vec()
    }

    fn write(&self, offset: usize, data: &[u8]) -> usize {
        let mut buf   = self.buffer.lock();
        if offset >= buf.len() { return 0; }
        let limit = core::cmp::min(offset + data.len(), buf.len());
        let write_len = limit - offset;
        buf[offset..limit].copy_from_slice(&data[0..write_len]);
        kprint!("[Driver fb0] Framebuffer updated: {} bytes at offset {}.\n", write_len, offset);
        write_len
    }
}

// ---------------------------------------------------------------------------
// /dev/hda  — ATA Primary Master Hard Drive
// ---------------------------------------------------------------------------

extern "C" {
    fn ata_read_sectors(lba: u32, sector_count: u8, dest: *mut u8);
    fn ata_write_sectors(lba: u32, sector_count: u8, src: *const u8);
}

pub struct AtaDriver;

impl CharDriver for AtaDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }

    fn read(&self, offset: usize, size: usize) -> Vec<u8> {
        let start_lba = (offset / 512) as u32;
        let end_lba = ((offset + size + 511) / 512) as u32;
        let sector_count = (end_lba - start_lba) as u8;
        
        let mut buf = vec![0u8; (sector_count as usize) * 512];
        unsafe {
            ata_read_sectors(start_lba, sector_count, buf.as_mut_ptr());
        }
        
        let start_idx = offset % 512;
        let end_idx = start_idx + size;
        buf[start_idx..end_idx].to_vec()
    }

    fn write(&self, offset: usize, data: &[u8]) -> usize {
        let start_lba = (offset / 512) as u32;
        let end_lba = ((offset + data.len() + 511) / 512) as u32;
        let sector_count = (end_lba - start_lba) as u8;
        
        // Read-Modify-Write if not aligned
        let mut buf = vec![0u8; (sector_count as usize) * 512];
        unsafe {
            ata_read_sectors(start_lba, sector_count, buf.as_mut_ptr());
        }
        
        let start_idx = offset % 512;
        let end_idx = start_idx + data.len();
        buf[start_idx..end_idx].copy_from_slice(data);
        
        unsafe {
            ata_write_sectors(start_lba, sector_count, buf.as_ptr());
        }
        data.len()
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
        dm.drivers.insert("/dev/hda".to_string(),     Box::new(AtaDriver));

        kprint!("[DriverManager] Registered: /dev/null, /dev/urandom, /dev/fb0, /dev/hda\n");
        dm
    }

    pub fn is_device(&self, path: &str) -> bool {
        self.drivers.contains_key(path)
    }

    pub fn read_device(&self, path: &str, offset: usize, size: usize) -> Option<Vec<u8>> {
        self.drivers.get(path).map(|drv| drv.read(offset, size))
    }

    pub fn write_device(&self, path: &str, offset: usize, data: &[u8]) -> Option<usize> {
        self.drivers.get(path).map(|drv| drv.write(offset, data))
    }
}
