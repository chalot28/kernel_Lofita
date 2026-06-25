// kernel/src/vasm.rs
// Written in Rust (no_std)
// Virtual Address Space Manager: FFI bindings to Zig physical allocator + pager.

use crate::token::Token;
use crate::capability::Capability;

extern "C" {
    pub fn phys_alloc(pages: usize) -> *mut u8;
    pub fn phys_free(ptr: *mut u8, pages: usize);
    pub fn page_table_map(virtual_addr: usize, physical_addr: usize, flags: u32) -> i32;
    pub fn page_table_unmap(virtual_addr: usize);
}

#[derive(Debug)]
pub struct Vma {
    pub start_addr:    usize,
    pub size:          usize,
    pub is_writeable:  bool,
    pub is_executable: bool,
    pub phys_ptr:      *mut u8,
}

// Raw pointer inside Vma: we guarantee single-threaded access via the kernel
// global mutex, so these impls are safe in our usage context.
unsafe impl Send for Vma {}
unsafe impl Sync for Vma {}

pub fn mem_alloc(token: &mut Token, req_cap: Capability, size: usize, vaddr: usize) -> Option<*mut u8> {
    if !token.capabilities.contains(req_cap) || !token.capabilities.contains(Capability::MEM_ALLOC) {
        return None;
    }
    if !token.check_quota(size) {
        return None;
    }
    // LAZY ALLOCATION: Do not allocate physical pages here.
    // Just register the VMA.
    let vma = Vma {
        start_addr: vaddr,
        size,
        is_writeable: true,
        is_executable: false,
        phys_ptr: core::ptr::null_mut(),
    };
    token.vmas.insert(vaddr, vma);
    token.memory_used += size;
    
    Some(vaddr as *mut u8)
}

#[no_mangle]
pub extern "C" fn vasm_handle_page_fault(fault_addr: usize, _error_code: u32) -> bool {
    use crate::kernel;
    let state = kernel().lock();
    
    // 1. Get active thread
    let active_thread_arc = match &state.scheduler.active_thread {
        Some(t) => alloc::sync::Arc::clone(t),
        None => return false,
    };
    
    let thread = active_thread_arc.lock();
    let session_id = thread.session_id;
    
    // 2. Get session -> token
    let session = match state.sessions.get(&session_id) {
        Some(s) => s,
        None => return false,
    };
    let mut token = session.token.lock();
    
    // 3. Find VMA containing fault_addr
    let mut found_vma_start = None;
    for (&start_addr, vma) in token.vmas.range(..=fault_addr).rev() {
        if fault_addr < start_addr + vma.size {
            found_vma_start = Some(start_addr);
            break; // Found the VMA!
        }
    }
    
    if let Some(start_addr) = found_vma_start {
        let vma = token.vmas.get_mut(&start_addr).unwrap();
        
        // 4. Lazy Physical Allocation
        if vma.phys_ptr.is_null() {
            let pages = (vma.size + 4095) / 4096;
            let phys = unsafe { phys_alloc(pages) };
            if phys.is_null() {
                return false; // Out of memory
            }
            vma.phys_ptr = phys;
            
            let mut flags: u32 = 1 | 4; // Present | User
            if vma.is_writeable { flags |= 2; }
            
            for i in 0..pages {
                let v_page = vma.start_addr + i * 4096;
                let p_page = phys as usize + i * 4096;
                unsafe { page_table_map(v_page, p_page, flags); }
            }
            return true; // Successfully demand-paged
        }
        // If it's already allocated, this might be a protection fault
        return false;
    }
    
    false
}
