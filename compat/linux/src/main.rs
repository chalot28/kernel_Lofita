// compat/linux/src/main.rs
// Written in Rust (no_std)
// Entry point for the Lofita Linux compatibility daemon.
// In bare-metal mode this is NOT a separate process — it is called
// as a kernel module from rust_kernel_init() when an ELF needs to run.
// This file exists for host-side testing (the run_simulation.py shim).

// When compiling for the bare-metal kernel, there is no main().
// When compiling for host testing, the simulation harness provides its own main.

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
extern crate alloc;

pub mod elf_loader;

pub mod syscall;
pub mod ffi;

#[cfg(not(test))]
use crate::kprint;

/// Called from the kernel when sys_execve is invoked with a path to an ELF binary.
/// `elf_data` must be a slice of bytes read from the VFS.
#[no_mangle]
pub extern "C" fn compat_linux_execve(elf_data_ptr: *const u8, elf_data_len: usize) -> i64 {
    let elf_data = unsafe { core::slice::from_raw_parts(elf_data_ptr, elf_data_len) };

    match elf_loader::load_elf(elf_data) {
        Ok(proc) => {
            #[cfg(not(test))]
            kprint!("[compat] ELF loaded. Entry=0x{:x}\n", proc.entry_point);
            // TODO: scheduler spawn with proc.entry_point and proc.initial_rsp
            proc.entry_point as i64
        }
        Err(e) => {
            #[cfg(not(test))]
            kprint!("[compat] ELF load error: {:?}\n", e);
            -1
        }
    }
}
