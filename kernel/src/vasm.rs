// kernel/src/vasm.rs
// Written in Rust (no_std)
// Virtual Address Space Manager: FFI bindings to Zig physical allocator + pager.
// No changes needed for no_std — this file uses only extern "C" declarations
// and a plain struct with raw pointers.

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
