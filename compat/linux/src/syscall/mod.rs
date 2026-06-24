// compat/linux/src/syscall/mod.rs
// Written in Rust (no_std)
// Syscall routing for the Linux compatibility layer.
//
// Linux x86_64 syscall numbers (from <asm/unistd_64.h>):
// https://github.com/torvalds/linux/blob/master/arch/x86/entry/syscalls/syscall_64.tbl
//
// This router maps syscall numbers to Lofita internal handlers.
// It mirrors WSL1's approach: translate Linux ABI → Lofita kernel calls.

pub mod memory;
pub mod fs;
pub mod net;
pub mod process;

use memory::MemorySyscalls;
use fs::FsSyscalls;
use net::NetSyscalls;
use process::ProcessSyscalls;
use crate::kprint;

// ─── FFI to Lofita Kernel Core ───────────────────────────────────────────────

extern "C" {
    fn rust_check_capability(session_id: u64, cap_val: u32) -> bool;
    fn rust_vfs_read(session_id: u64, fd: u32, buf: *mut u8, len: usize) -> i32;
    fn rust_vfs_write(session_id: u64, fd: u32, buf: *const u8, len: usize) -> i32;
    fn rust_vfs_open(session_id: u64, path: *const u8, len: usize, write: u32) -> u32;
    fn rust_vfs_close(session_id: u64, fd: u32) -> i32;
    fn rust_allocate_memory(token_id: u64, size: usize, writable: bool, exec: bool) -> usize;
    fn rust_run_process(token_id: u64, ticks: u64) -> u64;
    fn rust_create_child_token(
        caller_session: u64, name: *const u8, name_len: usize,
        priv_val: u32, parent_id: u64, mem_limit: usize,
        cap_flags: u32, lifetime_ticks: u64,
    ) -> u64;
}

// ─── Syscall Router ──────────────────────────────────────────────────────────

pub struct SyscallRouter {
    pub memory: MemorySyscalls,
}

impl SyscallRouter {
    pub fn new() -> Self {
        SyscallRouter {
            memory: MemorySyscalls::new(),
        }
    }

    /// Route a Linux syscall to the appropriate Lofita handler.
    ///
    /// Arguments match the Linux x86_64 syscall ABI:
    ///   nr  = syscall number (RAX)
    ///   args[0..5] = RDI, RSI, RDX, R10, R8, R9
    pub fn route(
        &mut self,
        nr:         u64,
        args:       [u64; 6],
        session_id: u64,
        token_id:   u64,
    ) -> i64 {
        match nr {
            // ── File I/O ───────────────────────────────────────────────────
            0 => {
                // sys_read(fd, buf, count)
                unsafe {
                    rust_vfs_read(session_id, args[0] as u32, args[1] as *mut u8, args[2] as usize) as i64
                }
            }
            1 => {
                // sys_write(fd, buf, count)
                // fd=1 (stdout) / fd=2 (stderr) → route to VGA via kernel
                unsafe {
                    rust_vfs_write(session_id, args[0] as u32, args[1] as *const u8, args[2] as usize) as i64
                }
            }
            2 => {
                // sys_open(path, flags, mode)
                let path_ptr = args[0] as *const u8;
                let path_len = args[1] as usize;
                if path_ptr.is_null() || (path_ptr as usize) < 0x1000 {
                    return -14; // EFAULT
                }
                let o_wronly = 1u64;
                let o_rdwr   = 2u64;
                let is_write = (args[2] & (o_wronly | o_rdwr)) != 0;
                unsafe {
                    rust_vfs_open(session_id, path_ptr, path_len, is_write as u32) as i64
                }
            }
            3 => {
                // sys_close(fd)
                unsafe { rust_vfs_close(session_id, args[0] as u32) as i64 }
            }
            // ── Memory ────────────────────────────────────────────────────
            9 => {
                // sys_mmap(addr, length, prot, flags, fd, offset)
                // PROT_EXEC=4, PROT_WRITE=2, PROT_READ=1
                let prot      = args[2];
                let is_write  = (prot & 2) != 0;
                let is_exec   = (prot & 4) != 0;
                unsafe {
                    rust_allocate_memory(token_id, args[1] as usize, is_write, is_exec) as i64
                }
            }
            11 => {
                // sys_munmap(addr, length) — stub (return success)
                0
            }
            12 => {
                // sys_brk(new_brk) — program break (heap expansion)
                self.memory.sys_brk(token_id, args[0])
            }
            // ── Process control ───────────────────────────────────────────
            57 => {
                // sys_fork() — create a child token
                let name = b"fork-child";
                let child_id = unsafe {
                    rust_create_child_token(
                        session_id,
                        name.as_ptr(), name.len(),
                        3,          // PrivilegeLevel::Process
                        token_id,
                        64 * 1024 * 1024, // 64MB limit
                        0b0001_1111, // MEM_ALLOC|MEM_FREE|FS_READ|FS_WRITE
                        100_000,     // 100k ticks lifetime
                    )
                };
                child_id as i64
            }
            59 => {
                // sys_execve(path, argv, envp) — load and execute ELF
                // In Phase 3 this calls elf_loader::load_elf()
                // For now we return ENOSYS to signal "not yet fully implemented"
                kprint!("[compat] sys_execve: path=0x{:x} (Phase 3 TODO: full ELF exec)\n", args[0]);
                -38 // ENOSYS
            }
            60 => {
                // sys_exit(status)
                ProcessSyscalls::sys_exit(0, args[0] as i64)
            }
            231 => {
                // sys_exit_group(status)
                ProcessSyscalls::sys_exit(0, args[0] as i64)
            }
            // ── Networking ────────────────────────────────────────────────
            41 => {
                // sys_socket(domain, type, protocol)
                let has_net = unsafe {
                    rust_check_capability(session_id, 1 << 4) // NET_CONNECT
                };
                NetSyscalls::sys_socket(0, has_net)
            }
            42 => {
                // sys_connect — stub
                if unsafe { !rust_check_capability(session_id, 1 << 4) } {
                    return -1; // EPERM
                }
                -38 // ENOSYS stub
            }
            // ── Time ──────────────────────────────────────────────────────
            228 => {
                // sys_clock_gettime — return dummy time
                0
            }
            // ── Process identity (commonly called at startup) ─────────────
            39  => 1,   // sys_getpid → PID 1
            102 => 0,   // sys_getuid → uid 0 (root)
            104 => 0,   // sys_getgid → gid 0
            107 => 0,   // sys_geteuid → euid 0
            108 => 0,   // sys_getegid → egid 0
            // ── ioctl ─────────────────────────────────────────────────────
            16 => {
                // sys_ioctl(fd, request, argp) — minimal stub
                -25 // ENOTTY (not a terminal)
            }
            // ── Fallthrough ───────────────────────────────────────────────
            nr => {
                kprint!("[compat] Unhandled syscall #{}\n", nr);
                -38 // ENOSYS
            }
        }
    }
}
