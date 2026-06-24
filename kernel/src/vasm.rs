// kernel/src/vasm.rs
// Written in Rust
// Virtual Address Space Manager and VMA descriptions.

extern "C" {
    pub fn phys_alloc(pages: usize) -> *mut u8;
    pub fn phys_free(ptr: *mut u8, pages: usize);
}

#[derive(Debug)]
pub struct Vma {
    pub start_addr: usize,
    pub size: usize,
    pub is_writeable: bool,
    pub is_executable: bool,
    pub phys_ptr: *mut u8,
}

unsafe impl Send for Vma {}
unsafe impl Sync for Vma {}
