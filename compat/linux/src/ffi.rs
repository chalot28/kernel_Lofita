// compat/linux/src/ffi.rs
// Written in Rust (no_std)
// C-ABI FFI declarations for calling Lofita kernel services from the compat layer.

extern "C" {
    /// Allocate `pages` contiguous physical pages. Returns null on failure.
    pub fn phys_alloc(pages: usize) -> *mut u8;
    /// Free previously allocated physical pages.
    pub fn phys_free(ptr: *mut u8, pages: usize);
    /// Map a virtual page to a physical page with given flags.
    /// flags: bit0=writable, bit1=user, bit2=executable
    pub fn page_table_map(virtual_addr: usize, physical_addr: usize, flags: u32) -> i32;
    /// Unmap a virtual page.
    pub fn page_table_unmap(virtual_addr: usize);
}
