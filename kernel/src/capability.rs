// kernel/src/capability.rs
// Written in Rust
// Core Capability definitions and Privilege checks.

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capability: u32 {
        const NONE         = 0;
        const MEM_ALLOC    = 1 << 0;
        const MEM_FREE     = 1 << 1;
        const FS_READ      = 1 << 2;
        const FS_WRITE     = 1 << 3;
        const NET_CONNECT  = 1 << 4;
        const NET_BIND     = 1 << 5;
        const SYS_ADMIN    = 1 << 6;
        const DRV_MMIO     = 1 << 7;
        const WINE_BRIDGE  = 1 << 8;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    Root = 0,
    Admin = 1,
    User = 2,
    Process = 3,
}
