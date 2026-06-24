// kernel/src/lib.rs
// Written in Rust (no_std)
// Core monolithic kernel policies and subsystem integrations.
//
// ═══════════════════════════════════════════════════════════
//  FREESTANDING CONFIGURATION
// ═══════════════════════════════════════════════════════════
#![no_std]

// Allow heap-allocated types (Vec, String, Box, Arc) through alloc crate.
// The actual heap is backed by our linked_list_allocator below.
extern crate alloc;

pub mod capability;
pub mod token;
pub mod session;
pub mod sched;
pub mod vasm;
pub mod vfs;
pub mod ipc;
pub mod compress;
pub mod web;
pub mod driver;

use alloc::sync::Arc;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::mem;
use spin::Mutex;
use hashbrown::HashMap;

use capability::{Capability, PrivilegeLevel};
use token::Token;
use session::Session;
use vasm::{Vma, phys_alloc, phys_free, page_table_map, page_table_unmap};
use sched::{Scheduler, ThreadState};
use vfs::VfsState;
use ipc::IpcSubsystem;
use driver::DriverManager;

// ═══════════════════════════════════════════════════════════
//  GLOBAL HEAP ALLOCATOR
//  Uses linked_list_allocator over a static 4MB kernel heap.
// ═══════════════════════════════════════════════════════════

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// 4MB kernel heap (in BSS — zeroed at boot)
static mut HEAP_MEM: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024];

/// Must be called ONCE before any heap allocation.
/// Called from rust_kernel_init() as the very first step.
unsafe fn heap_init() {
    let heap_start = core::ptr::addr_of_mut!(HEAP_MEM) as *mut u8;
    let heap_size  = 4 * 1024 * 1024;
    ALLOCATOR.lock().init(heap_start, heap_size);
}

// ═══════════════════════════════════════════════════════════
//  KERNEL OUTPUT MACRO
//  kprint! routes to the Zig VGA driver via FFI.
// ═══════════════════════════════════════════════════════════

extern "C" {
    /// Defined in drivers/vga.zig — prints a byte slice to VGA buffer.
    fn vga_print_bytes(ptr: *const u8, len: usize);
}

/// VGA kernel print — formats via core::fmt, then calls the Zig VGA driver.
pub fn kprint_str(s: &str) {
    unsafe { vga_print_bytes(s.as_ptr(), s.len()); }
}

/// Internal helper used by the kprint! macro.
pub struct KWriter;

impl core::fmt::Write for KWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        kprint_str(s);
        Ok(())
    }
}

/// kprint!(format_str, args...) — like println! but to the VGA buffer.
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(crate::KWriter, $($arg)*);
    }};
}

// ═══════════════════════════════════════════════════════════
//  KERNEL GLOBAL STATE
//  Protected by a spin::Mutex (no OS scheduler needed).
// ═══════════════════════════════════════════════════════════

pub struct KernelState {
    pub tokens:          HashMap<u64, Arc<Mutex<Token>>>,
    pub sessions:        HashMap<u64, Session>,
    pub scheduler:       Scheduler,
    pub vfs:             VfsState,
    pub ipc:             IpcSubsystem,
    pub driver:          DriverManager,
    pub next_token_id:   u64,
    pub next_session_id: u64,
    /// Monotonically increasing kernel tick counter (incremented by timer ISR).
    pub tick_count:      u64,
}

/// Global kernel state — initialized lazily on first lock().
/// spin::Once ensures it's initialized exactly once without std runtime.
static KERNEL_ONCE: spin::Once<Mutex<KernelState>> = spin::Once::new();

pub fn kernel() -> &'static Mutex<KernelState> {
    KERNEL_ONCE.call_once(|| {
        Mutex::new(KernelState {
            tokens:          HashMap::new(),
            sessions:        HashMap::new(),
            scheduler:       Scheduler::new(),
            vfs:             VfsState::new(),
            ipc:             IpcSubsystem::new(),
            driver:          DriverManager::new(),
            next_token_id:   1,
            next_session_id: 1,
            tick_count:      0,
        })
    })
}

// ═══════════════════════════════════════════════════════════
//  PANIC HANDLER  (required by #![no_std])
// ═══════════════════════════════════════════════════════════

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    kprint!("\n*** KERNEL PANIC ***\n");
    if let Some(loc) = info.location() {
        kprint!("  at {}:{}\n", loc.file(), loc.line());
    }
    if let Some(msg) = info.message().as_str() {
        kprint!("  msg: {}\n", msg);
    }
    loop {
        unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)); }
    }
}

// ═══════════════════════════════════════════════════════════
//  STATIC INITRAMFS (RLE-compressed)
// ═══════════════════════════════════════════════════════════

static INITRAMFS_COMPRESSED: &[u8] = &[
    1, b'F', 1, b'I', 1, b'L', 1, b'E', 1, b':', 1, b'/', 1, b'e', 1, b't', 1, b'c', 1, b'/',
    1, b'v', 1, b'e', 1, b'r', 1, b's', 1, b'i', 1, b'o', 1, b'n', 1, b'\n',
    1, b'L', 1, b'o', 1, b'r', 1, b'i', 1, b'f', 1, b'a', 1, b' ', 1, b'M', 1, b'o', 1, b'n',
    1, b'o', 1, b'l', 1, b'i', 1, b't', 1, b'h', 1, b'i', 1, b'c', 1, b' ', 1, b'K', 1, b'e',
    1, b'r', 1, b'n', 1, b'e', 1, b'l', 1, b' ', 1, b'v', 1, b'1', 1, b'.', 1, b'0', 1, b'.', 1, b'0', 1, b'\n',
    1, b'F', 1, b'I', 1, b'L', 1, b'E', 1, b':', 1, b'/', 1, b'e', 1, b't', 1, b'c', 1, b'/',
    1, b'm', 1, b'o', 1, b't', 1, b'd', 1, b'\n',
    1, b'W', 1, b'e', 1, b'l', 1, b'c', 1, b'o', 1, b'm', 1, b'e', 1, b' ', 1, b't', 1, b'o',
    1, b' ', 1, b'L', 1, b'o', 1, b'r', 1, b'i', 1, b'f', 1, b'a', 1, b' ', 1, b'O', 1, b'S', 1, b'!', 1, b'\n',
    1, b'F', 1, b'I', 1, b'L', 1, b'E', 1, b':', 1, b'/', 1, b'e', 1, b't', 1, b'c', 1, b'/',
    1, b'h', 1, b'o', 1, b's', 1, b't', 1, b's', 1, b'\n',
    1, b'1', 1, b'2', 1, b'7', 1, b'.', 1, b'0', 1, b'.', 1, b'0', 1, b'.', 1, b'1', 1, b' ',
    1, b'l', 1, b'o', 1, b'c', 1, b'a', 1, b'l', 1, b'h', 1, b'o', 1, b's', 1, b't', 1, b'\n',
];

// ═══════════════════════════════════════════════════════════
//  C-ABI EXPORTS (called from Zig)
// ═══════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn rust_kernel_init() {
    // 1. Initialize heap FIRST — all subsequent code needs it
    unsafe { heap_init(); }
    kprint!("[kernel/rust] Heap initialized (4MB).\n");

    let mut state = kernel().lock();

    // 2. Create Root Token
    let root_token = Arc::new(Mutex::new(Token {
        id:            state.next_token_id,
        name:          "RootSystem".to_string(),
        privilege:     PrivilegeLevel::Root,
        parent:        None,
        children:      Vec::new(),
        vmas:          Vec::new(),
        memory_limit:  usize::MAX,
        memory_used:   0,
        is_permanent:  true,
        expiry_tick:   u64::MAX,
        capabilities:  Capability::all(),
        run_count:     0,
        is_deprecated: false,
    }));

    // Extract ID before the mutable insert to satisfy the borrow checker
    let root_id = state.next_token_id;
    state.tokens.insert(root_id, root_token);
    state.next_token_id += 1;
    kprint!("[kernel/rust] Root Token initialized.\n");

    // 3. Load initramfs into VFS
    kprint!("[kernel/rust] Loading initramfs ({} bytes)...\n", INITRAMFS_COMPRESSED.len());
    match compress::decompress(INITRAMFS_COMPRESSED) {
        Ok(decompressed) => {
            state.vfs.populate_from_initramfs(&decompressed);
        }
        Err(e) => {
            kprint!("[kernel/rust] initramfs error: {}\n", e);
        }
    }

    kprint!("[kernel/rust] Core subsystems ready.\n");
}

#[no_mangle]
pub extern "C" fn rust_create_child_token(
    caller_session_id: u64,
    name_ptr:          *const u8,
    name_len:          usize,
    privilege_val:     u32,
    parent_id:         u64,
    mem_limit:         usize,
    cap_flags:         u32,
    lifetime_ticks:    u64,
) -> u64 {
    let name_slice = unsafe { core::slice::from_raw_parts(name_ptr, name_len) };
    let name = core::str::from_utf8(name_slice).unwrap_or("unknown");

    let privilege = match privilege_val {
        0 => PrivilegeLevel::Root,
        1 => PrivilegeLevel::Admin,
        2 => PrivilegeLevel::User,
        _ => PrivilegeLevel::Process,
    };

    let cap = Capability::from_bits_truncate(cap_flags);
    let mut state = kernel().lock();

    // Verify caller owns the parent token
    let caller_valid = state.sessions.get(&caller_session_id)
        .map_or(false, |s| s.token.lock().id == parent_id);
    if !caller_valid {
        kprint!("[Security] Caller session does not own parent token.\n");
        return 0;
    }

    // Privilege and capability inheritance checks
    if let Some(parent_arc) = state.tokens.get(&parent_id) {
        let parent = parent_arc.lock();
        if (parent.privilege as u32) > privilege_val {
            kprint!("[Security] Privilege escalation attempt blocked.\n");
            return 0;
        }
        if (parent.capabilities.bits() & cap_flags) != cap_flags {
            kprint!("[Security] Capability overgrant attempt blocked.\n");
            return 0;
        }
    }

    let parent_weak = state.tokens.get(&parent_id)
        .map(|arc| Arc::downgrade(arc));
    let token_id = state.next_token_id;
    let expiry   = state.tick_count + lifetime_ticks;

    let new_token = Arc::new(Mutex::new(Token {
        id:            token_id,
        name:          name.to_string(),
        privilege,
        parent:        parent_weak,
        children:      Vec::new(),
        vmas:          Vec::new(),
        memory_limit:  mem_limit,
        memory_used:   0,
        is_permanent:  false,
        expiry_tick:   expiry,
        capabilities:  cap,
        run_count:     0,
        is_deprecated: false,
    }));

    if let Some(parent_arc) = state.tokens.get(&parent_id) {
        parent_arc.lock().children.push(Arc::clone(&new_token));
    }

    state.tokens.insert(token_id, new_token);
    state.next_token_id += 1;
    token_id
}

#[no_mangle]
pub extern "C" fn rust_run_process(token_id: u64, session_ticks: u64) -> u64 {
    let mut state = kernel().lock();
    let token_arc = match state.tokens.get(&token_id) {
        Some(arc) => Arc::clone(arc),
        None => return 0,
    };

    let mut token = token_arc.lock();
    if token.run_count >= 2 {
        return 0;
    }
    token.run_count += 1;

    let s_id   = state.next_session_id;
    state.next_session_id += 1;
    let expiry = state.tick_count + session_ticks;

    let session = Session {
        id:           s_id,
        token:        Arc::clone(&token_arc),
        expiry_tick:  expiry,
        is_active:    true,
    };

    state.sessions.insert(s_id, session);
    state.scheduler.spawn(s_id, &token.name, 0x1000_0000);
    s_id
}

fn get_aslr_offset(seed: usize) -> usize {
    let a: usize = 1_103_515_245;
    let c: usize = 12_345;
    let m: usize = 1 << 31;
    let rand = (a.wrapping_mul(seed).wrapping_add(c)) % m;
    (rand % 1024) * 4096 // Page-aligned, max 4MB
}

#[no_mangle]
pub extern "C" fn rust_allocate_memory(
    token_id:      u64,
    size:          usize,
    is_writeable:  bool,
    is_executable: bool,
) -> usize {
    let state = kernel().lock();
    let token_arc = match state.tokens.get(&token_id) {
        Some(arc) => Arc::clone(arc),
        None => return 0,
    };

    let mut token = token_arc.lock();
    let pages       = (size + 4095) / 4096;
    let actual_size = pages * 4096;

    // Enforce W^X policy
    if is_writeable && is_executable {
        kprint!("[Security] W^X violation blocked.\n");
        return 0;
    }

    let phys_ptr = unsafe { phys_alloc(pages) };
    if phys_ptr.is_null() { return 0; }

    let aslr_offset  = get_aslr_offset(token.id as usize ^ token.memory_used);
    let virtual_addr = 0x2000_0000 + (token.id as usize * 0x100_0000) + token.memory_used + aslr_offset;

    for i in 0..pages {
        let v_page = virtual_addr + i * 4096;
        let p_page = phys_ptr as usize + i * 4096;
        unsafe {
            let mut flags: u32 = 1;                              // Present
            if is_writeable  { flags |= 2; }
            if is_executable { flags |= 4; }
            page_table_map(v_page, p_page, flags);
        }
    }

    token.vmas.push(Vma {
        start_addr:    virtual_addr,
        size:          actual_size,
        is_writeable,
        is_executable,
        phys_ptr,
    });
    token.memory_used += actual_size;
    virtual_addr
}

#[no_mangle]
pub extern "C" fn rust_check_capability(session_id: u64, cap_val: u32) -> bool {
    let state   = kernel().lock();
    let session = match state.sessions.get(&session_id) {
        Some(s) => s,
        None => return false,
    };

    if !session.is_active || state.tick_count > session.expiry_tick {
        return false;
    }

    let token = session.token.lock();
    let cap   = Capability::from_bits_truncate(cap_val);
    token.capabilities.contains(cap)
}

#[no_mangle]
pub extern "C" fn rust_vfs_open(
    session_id: u64,
    path_ptr:   *const u8,
    path_len:   usize,
    write_val:  u32,
) -> u32 {
    let path_slice = unsafe { core::slice::from_raw_parts(path_ptr, path_len) };
    let path       = core::str::from_utf8(path_slice).unwrap_or("unknown");
    let is_write   = write_val != 0;

    let required_cap = if is_write {
        Capability::FS_WRITE.bits()
    } else {
        Capability::FS_READ.bits()
    };

    if !rust_check_capability(session_id, required_cap) {
        return 0;
    }

    let mut state = kernel().lock();
    match state.vfs.open(session_id, path, is_write) {
        Ok(fd) => fd,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "C" fn rust_vfs_close(session_id: u64, fd: u32) -> i32 {
    let mut state = kernel().lock();
    match state.vfs.close(session_id, fd) {
        Ok(_)  => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rust_vfs_read(
    session_id: u64,
    fd:         u32,
    buf_ptr:    *mut u8,
    buf_len:    usize,
) -> i32 {
    if !rust_check_capability(session_id, Capability::FS_READ.bits()) {
        return -13; // EACCES
    }

    // Bounds check on user buffer
    let buf_end = (buf_ptr as usize).checked_add(buf_len);
    if buf_ptr.is_null() || (buf_ptr as usize) < 0x2000_0000
       || buf_end.is_none() || buf_end.unwrap() > 0x8000_0000
    {
        return -14; // EFAULT
    }

    let state = kernel().lock();
    let dm    = &state.driver;
    match state.vfs.read(session_id, fd, buf_len, dm) {
        Ok(data) => {
            let copy_len = core::cmp::min(data.len(), buf_len);
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr(), buf_ptr, copy_len);
            }
            copy_len as i32
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rust_vfs_write(
    session_id: u64,
    fd:         u32,
    data_ptr:   *const u8,
    data_len:   usize,
) -> i32 {
    if !rust_check_capability(session_id, Capability::FS_WRITE.bits()) {
        return -13;
    }

    let data_end = (data_ptr as usize).checked_add(data_len);
    if data_ptr.is_null() || (data_ptr as usize) < 0x2000_0000
       || data_end.is_none() || data_end.unwrap() > 0x8000_0000
    {
        return -14;
    }

    let data = unsafe { core::slice::from_raw_parts(data_ptr, data_len) };
    let mut state = kernel().lock();
    // Split the borrow: get a raw pointer to driver so we can also borrow vfs mutably.
    // Safety: driver is never mutated during vfs.write(), both fields are disjoint.
    let dm_ptr: *const driver::DriverManager = &state.driver;
    let result = state.vfs.write(session_id, fd, data, unsafe { &*dm_ptr });
    match result {
        Ok(n)  => n as i32,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rust_ipc_send(
    port_id:    u32,
    session_id: u64,
    msg_ptr:    *const u8,
    msg_len:    usize,
) -> u32 {
    let msg_slice = unsafe { core::slice::from_raw_parts(msg_ptr, msg_len) };
    let msg       = core::str::from_utf8(msg_slice).unwrap_or("");

    let mut state    = kernel().lock();
    let woken_thread = state.ipc.send(port_id, session_id, msg);

    if let Some(tid) = woken_thread {
        for t_arc in &state.scheduler.threads {
            let mut t = t_arc.lock();
            if t.id == tid {
                t.state = ThreadState::Ready;
            }
        }
        tid
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn rust_ipc_recv(port_id: u32, receiver_thread_id: u32) -> i32 {
    let mut state = kernel().lock();
    match state.ipc.recv(port_id, receiver_thread_id) {
        Ok(Some(_msg)) => 1,
        Ok(None) => {
            for t_arc in &state.scheduler.threads {
                let mut t = t_arc.lock();
                if t.id == receiver_thread_id {
                    t.state = ThreadState::Blocked;
                }
            }
            0
        }
        Err(_) => -1,
    }
}

/// Called from the timer interrupt handler (IDT vector 32).
#[no_mangle]
pub extern "C" fn rust_kernel_tick() {
    let mut state = kernel().lock();
    state.tick_count += 1;
    let current_tick = state.tick_count;

    let mut expired: Vec<u64> = Vec::new();
    for (&t_id, token_arc) in &state.tokens {
        let token = token_arc.lock();
        if token.is_expired(current_tick) {
            expired.push(t_id);
        }
    }

    for t_id in expired {
        kprint!("[kernel/rust] Token {} expired.\n", t_id);
        if let Some(token_arc) = state.tokens.remove(&t_id) {
            let mut token = token_arc.lock();
            let vmas      = mem::take(&mut token.vmas);
            for vma in vmas {
                let pages = vma.size / 4096;
                for i in 0..pages {
                    unsafe { page_table_unmap(vma.start_addr + i * 4096); }
                }
                unsafe { phys_free(vma.phys_ptr, pages); }
            }
        }
    }

    state.scheduler.schedule_next();
}

/// Entry point for Linux-compat syscall routing (called from IDT int 0x80 handler).
#[no_mangle]
pub extern "C" fn rust_syscall_dispatch(
    nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, _a5: u64, _a6: u64,
) -> i64 {
    match nr {
        // sys_read
        0 => rust_vfs_read(a1, a2 as u32, a3 as *mut u8, a4 as usize) as i64,
        // sys_write
        1 => rust_vfs_write(a1, a2 as u32, a3 as *const u8, a4 as usize) as i64,
        // sys_open — stub (requires path from user address space)
        2 => rust_vfs_open(0, a1 as *const u8, a2 as usize, a3 as u32) as i64,
        // sys_close
        3 => rust_vfs_close(a1, a2 as u32) as i64,
        // sys_exit / sys_exit_group
        60 | 231 => {
            kprint!("[syscall] Process exit({}).\n", a1);
            -1
        }
        nr => {
            kprint!("[syscall] Unhandled syscall #{}\n", nr);
            -38 // ENOSYS
        }
    }
}
