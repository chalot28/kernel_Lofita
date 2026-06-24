// kernel/src/lib.rs
// Written in Rust
// Core monolithic policies and kernel integrations.

pub mod capability;
pub mod token;
pub mod session;
pub mod sched;
pub mod vasm;
pub mod vfs;
pub mod ipc;
pub mod compress;
pub mod web;


use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use capability::{Capability, PrivilegeLevel};
use token::Token;
use session::Session;
use vasm::{Vma, phys_alloc, phys_free};
use sched::{Scheduler, ThreadState};
use vfs::VfsState;
use ipc::IpcSubsystem;

pub struct KernelState {
    pub tokens: HashMap<u64, Arc<Mutex<Token>>>,
    pub sessions: HashMap<u64, Session>,
    pub scheduler: Scheduler,
    pub vfs: VfsState,
    pub ipc: IpcSubsystem,
    pub next_token_id: u64,
    pub next_session_id: u64,
}

lazy_static::lazy_static! {
    pub static ref KERNEL: Mutex<KernelState> = Mutex::new(KernelState {
        tokens: HashMap::new(),
        sessions: HashMap::new(),
        scheduler: Scheduler::new(),
        vfs: VfsState::new(),
        ipc: IpcSubsystem::new(),
        next_token_id: 1,
        next_session_id: 1,
    });
}

// C-ABI FFI bindings called by Zig/loader code

#[no_mangle]
pub extern "C" fn rust_kernel_init() {
    let mut state = KERNEL.lock().unwrap();
    
    // Create Root Token (id 1)
    let root_token = Arc::new(Mutex::new(Token {
        id: state.next_token_id,
        name: "RootSystem".to_string(),
        privilege: PrivilegeLevel::Root,
        parent: None,
        children: Vec::new(),
        vmas: Vec::new(),
        memory_limit: usize::MAX,
        memory_used: 0,
        is_permanent: true,
        expiry: Instant::now() + Duration::from_secs(999999),
        capabilities: Capability::all(),
        run_count: 0,
        is_deprecated: false,
    }));
    
    state.tokens.insert(state.next_token_id, root_token);
    state.next_token_id += 1;
    
    println!("[kernel/rust-core] Monolithic Root Token initialized successfully.");
}

#[no_mangle]
pub extern "C" fn rust_create_child_token(
    name_ptr: *const u8,
    name_len: usize,
    privilege_val: u32,
    parent_id: u64,
    mem_limit: usize,
    cap_flags: u32,
    lifetime_secs: u64,
) -> u64 {
    let name_slice = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };
    let name = std::str::from_utf8(name_slice).unwrap_or("unknown");

    let privilege = match privilege_val {
        0 => PrivilegeLevel::Root,
        1 => PrivilegeLevel::Admin,
        2 => PrivilegeLevel::User,
        _ => PrivilegeLevel::Process,
    };

    let cap = Capability::from_bits_truncate(cap_flags);
    let mut state = KERNEL.lock().unwrap();

    let parent_weak = state.tokens.get(&parent_id).map(|arc| Arc::downgrade(arc));
    let token_id = state.next_token_id;
    
    let new_token = Arc::new(Mutex::new(Token {
        id: token_id,
        name: name.to_string(),
        privilege,
        parent: parent_weak,
        children: Vec::new(),
        vmas: Vec::new(),
        memory_limit: mem_limit,
        memory_used: 0,
        is_permanent: false,
        expiry: Instant::now() + Duration::from_secs(lifetime_secs),
        capabilities: cap,
        run_count: 0,
        is_deprecated: false,
    }));

    if let Some(parent_arc) = state.tokens.get(&parent_id) {
        parent_arc.lock().unwrap().children.push(Arc::clone(&new_token));
    }

    state.tokens.insert(token_id, new_token);
    state.next_token_id += 1;
    token_id
}

#[no_mangle]
pub extern "C" fn rust_run_process(token_id: u64, session_secs: u64) -> u64 {
    let mut state = KERNEL.lock().unwrap();
    let token_arc = match state.tokens.get(&token_id) {
        Some(arc) => Arc::clone(arc),
        None => return 0,
    };

    let mut token = token_arc.lock().unwrap();
    if token.run_count >= 2 {
        return 0;
    }
    token.run_count += 1;

    let s_id = state.next_session_id;
    state.next_session_id += 1;

    let session = Session {
        id: s_id,
        token: Arc::clone(&token_arc),
        expiry: Instant::now() + Duration::from_secs(session_secs),
        is_active: true,
    };

    state.sessions.insert(s_id, session);
    state.scheduler.spawn(s_id, &token.name, 0x10000000);
    s_id
}

#[no_mangle]
pub extern "C" fn rust_allocate_memory(token_id: u64, size: usize) -> usize {
    let mut state = KERNEL.lock().unwrap();
    let token_arc = match state.tokens.get(&token_id) {
        Some(arc) => Arc::clone(arc),
        None => return 0,
    };

    let mut token = token_arc.lock().unwrap();
    let pages = (size + 4095) / 4096;
    let actual_size = pages * 4096;

    let phys_ptr = unsafe { phys_alloc(pages) };
    if phys_ptr.is_null() {
        return 0;
    }

    let virtual_addr = 0x20000000 + (token.id as usize * 0x1000000) + token.memory_used;
    let vma = Vma {
        start_addr: virtual_addr,
        size: actual_size,
        is_writeable: true,
        is_executable: false,
        phys_ptr,
    };

    token.vmas.push(vma);
    token.memory_used += actual_size;
    virtual_addr
}

#[no_mangle]
pub extern "C" fn rust_check_capability(session_id: u64, cap_val: u32) -> bool {
    let state = KERNEL.lock().unwrap();
    let session = match state.sessions.get(&session_id) {
        Some(s) => s,
        None => return false,
    };

    if !session.is_active || Instant::now() > session.expiry {
        return false;
    }

    let token = session.token.lock().unwrap();
    let cap = Capability::from_bits_truncate(cap_val);
    token.capabilities.contains(cap)
}

#[no_mangle]
pub extern "C" fn rust_vfs_open(session_id: u64, path_ptr: *const u8, path_len: usize, write_val: u32) -> u32 {
    let path_slice = unsafe { std::slice::from_raw_parts(path_ptr, path_len) };
    let path = std::str::from_utf8(path_slice).unwrap_or("unknown");
    let is_write = write_val != 0;

    let mut state = KERNEL.lock().unwrap();
    match state.vfs.open(session_id, path, is_write) {
        Ok(fd) => fd,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "C" fn rust_vfs_close(session_id: u64, fd: u32) -> i32 {
    let mut state = KERNEL.lock().unwrap();
    match state.vfs.close(session_id, fd) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rust_ipc_send(port_id: u32, session_id: u64, msg_ptr: *const u8, msg_len: usize) -> u32 {
    let msg_slice = unsafe { std::slice::from_raw_parts(msg_ptr, msg_len) };
    let msg = std::str::from_utf8(msg_slice).unwrap_or("");

    let mut state = KERNEL.lock().unwrap();
    let woken_thread = state.ipc.send(port_id, session_id, msg);

    if let Some(tid) = woken_thread {
        // Change woken thread status from Blocked to Ready
        for t_arc in &state.scheduler.threads {
            let mut t = t_arc.lock().unwrap();
            if t.id == tid {
                t.state = ThreadState::Ready;
                println!("[Scheduler] Woke up Thread {} from Blocked state.", tid);
            }
        }
        tid
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn rust_ipc_recv(port_id: u32, receiver_thread_id: u32) -> i32 {
    let mut state = KERNEL.lock().unwrap();
    match state.ipc.recv(port_id, receiver_thread_id) {
        Ok(Some(msg)) => {
            println!("[IPC FFI] Thread {} read message: \"{}\"", receiver_thread_id, msg.payload);
            1 // Message read successfully
        }
        Ok(None) => {
            // Block thread in Scheduler
            for t_arc in &state.scheduler.threads {
                let mut t = t_arc.lock().unwrap();
                if t.id == receiver_thread_id {
                    t.state = ThreadState::Blocked;
                }
            }
            0 // Blocked
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rust_kernel_tick() {
    let mut state = KERNEL.lock().unwrap();
    let now = Instant::now();
    let mut expired_tokens = Vec::new();

    for (&t_id, token_arc) in &state.tokens {
        let token = token_arc.lock().unwrap();
        if token.is_expired(now) {
            expired_tokens.push(t_id);
        }
    }

    for t_id in expired_tokens {
        println!("[kernel/rust-core] Token {} expired. Commencing virtual unmappings...", t_id);
        if let Some(token_arc) = state.tokens.remove(&t_id) {
            let mut token = token_arc.lock().unwrap();
            let vmas = std::mem::take(&mut token.vmas);
            for vma in vmas {
                let pages = vma.size / 4096;
                unsafe {
                    phys_free(vma.phys_ptr, pages);
                }
            }
        }
    }

    state.scheduler.schedule_next();
}
