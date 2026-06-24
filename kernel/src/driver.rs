// kernel/src/driver.rs
// Written in Rust
// Monolithic Char Device Driver manager and drivers for null, urandom, fb0.

use std::collections::HashMap;

pub trait CharDriver: Send + Sync {
    fn open(&self) -> Result<(), &'static str>;
    fn read(&self, size: usize) -> Vec<u8>;
    fn write(&self, data: &[u8]) -> usize;
}

pub struct NullDriver;
impl CharDriver for NullDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }
    fn read(&self, _size: usize) -> Vec<u8> { Vec::new() } // returns EOF
    fn write(&self, data: &[u8]) -> usize { data.len() } // discards, returns success
}

pub struct UrandomDriver;
impl CharDriver for UrandomDriver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }
    
    fn read(&self, size: usize) -> Vec<u8> {
        // Simple mock random byte generator
        let mut bytes = Vec::with_capacity(size);
        for i in 0..size {
            bytes.push(((i * 33 + 7) % 256) as u8);
        }
        bytes
    }
    
    fn write(&self, data: &[u8]) -> usize { data.len() } // discards
}

pub struct Fb0Driver {
    pub buffer: std::sync::Mutex<Vec<u8>>,
}
impl CharDriver for Fb0Driver {
    fn open(&self) -> Result<(), &'static str> { Ok(()) }
    
    fn read(&self, size: usize) -> Vec<u8> {
        let buf = self.buffer.lock().unwrap();
        let limit = std::cmp::min(size, buf.len());
        buf[0..limit].to_vec()
    }
    
    fn write(&self, data: &[u8]) -> usize {
        let mut buf = self.buffer.lock().unwrap();
        let limit = std::cmp::min(data.len(), buf.len());
        buf[0..limit].copy_from_slice(&data[0..limit]);
        println!("[Driver fb0] Framebuffer buffer updated with {} bytes.", limit);
        limit
    }
}

pub struct DriverManager {
    pub drivers: HashMap<String, Box<dyn CharDriver>>,
}

impl DriverManager {
    pub fn new() -> Self {
        let mut dm = DriverManager {
            drivers: HashMap::new(),
        };
        
        // Register core OS drivers
        dm.drivers.insert("/dev/null".to_string(), Box::new(NullDriver));
        dm.drivers.insert("/dev/urandom".to_string(), Box::new(UrandomDriver));
        dm.drivers.insert("/dev/fb0".to_string(), Box::new(Fb0Driver {
            buffer: std::sync::Mutex::new(vec![0; 4096]), // 4KB mock screen pixels
        }));
        
        println!("[DriverManager] Registered character devices: /dev/null, /dev/urandom, /dev/fb0");
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
