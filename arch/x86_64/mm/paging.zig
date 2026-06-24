// arch/x86_64/mm/paging.zig — Lofita OS x86_64 Page Table Manager
// Written in Zig (freestanding — no std, no libc)
//
// Manages 4-level x86_64 page tables (PML4 → PDPT → PD → PT).
// Each level has 512 entries × 8 bytes = 4096 bytes (one page).
//
// Physical page allocation is delegated to mm/ppa.zig (buddy allocator).
//
// Page Table Entry (PTE) bit flags (Intel Vol. 3A §4.5):
//   Bit 0  P   — Present
//   Bit 1  R/W — Read/Write (0=read-only)
//   Bit 2  U/S — User/Supervisor (1=user accessible)
//   Bit 3  PWT — Page-level write-through
//   Bit 4  PCD — Page-level cache disable
//   Bit 5  A   — Accessed (set by CPU)
//   Bit 6  D   — Dirty (set by CPU, PTE only)
//   Bit 7  PS  — Page Size (PD/PDPT huge pages)
//   Bits 12-51 Physical address (4KB-aligned → bits 11:0 = 0)
//   Bit 63 NX  — No-Execute (requires EFER.NXE=1)

const vga = @import("../../drivers/vga.zig");
const ppa = @import("../../mm/ppa.zig");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const PAGE_SIZE:  usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const PT_ENTRIES: usize = 512;

/// Page entry flags exposed to callers
pub const PF_PRESENT:    u64 = 1 << 0;
pub const PF_WRITABLE:   u64 = 1 << 1;
pub const PF_USER:       u64 = 1 << 2;
pub const PF_NO_EXECUTE: u64 = 1 << 63;

// Mask to extract 4KB-aligned physical address from a PTE
const PHYS_ADDR_MASK: u64 = 0x000FFFFF_FFFFF000;

// ---------------------------------------------------------------------------
// Raw page table (512 × u64 entries, must be 4096-byte aligned)
// ---------------------------------------------------------------------------

const RawTable = struct {
    entries: [PT_ENTRIES]u64 align(PAGE_SIZE),

    fn zeroed() RawTable {
        return .{ .entries = [_]u64{0} ** PT_ENTRIES };
    }

    fn get_phys(self: *const RawTable) u64 {
        return @intFromPtr(&self.entries);
    }
};

// ---------------------------------------------------------------------------
// Virtual-address decomposition into 4-level indices
// ---------------------------------------------------------------------------

const VaIndices = struct {
    pml4: usize,
    pdpt: usize,
    pd:   usize,
    pt:   usize,
    off:  usize,
};

fn va_split(vaddr: u64) VaIndices {
    return VaIndices{
        .pml4 = @intCast((vaddr >> 39) & 0x1FF),
        .pdpt = @intCast((vaddr >> 30) & 0x1FF),
        .pd   = @intCast((vaddr >> 21) & 0x1FF),
        .pt   = @intCast((vaddr >> 12) & 0x1FF),
        .off  = @intCast(vaddr & 0xFFF),
    };
}

// ---------------------------------------------------------------------------
// PagingContext — one per address space (kernel or process)
// ---------------------------------------------------------------------------

pub const PagingContext = struct {
    pml4: *RawTable,

    /// Create a new, empty address space.
    /// Allocates one page for the PML4 from the physical page allocator.
    pub fn create() ?PagingContext {
        const raw = ppa.phys_alloc(1) orelse return null;
        const table: *RawTable = @alignCast(@ptrCast(raw));
        table.* = RawTable.zeroed();
        return PagingContext{ .pml4 = table };
    }

    /// Activate this address space by loading its PML4 into CR3.
    pub fn activate(self: *const PagingContext) void {
        const cr3_val = self.pml4.get_phys();
        asm volatile ("movq %[cr3], %%cr3"
            :
            : [cr3] "r" (cr3_val)
            : "memory"
        );
    }

    /// Map `virtual_addr` → `physical_addr` with the given flags.
    /// Allocates intermediate table pages as needed.
    ///
    /// `flags`: combination of PF_PRESENT | PF_WRITABLE | PF_USER | PF_NO_EXECUTE
    pub fn map(self: *PagingContext, virtual_addr: u64, physical_addr: u64, flags: u64) !void {
        const va = va_split(virtual_addr);

        // ---- PML4 → PDPT ----
        const pdpt = try ensure_table(&self.pml4.entries[va.pml4]);

        // ---- PDPT → PD ----
        const pd = try ensure_table(&pdpt.entries[va.pdpt]);

        // ---- PD → PT ----
        const pt = try ensure_table(&pd.entries[va.pd]);

        // ---- PT → Physical page ----
        pt.entries[va.pt] = (physical_addr & PHYS_ADDR_MASK) | flags | PF_PRESENT;

        // Invalidate the TLB entry for this virtual address
        invlpg(virtual_addr);
    }

    /// Unmap a single virtual page.
    pub fn unmap(self: *PagingContext, virtual_addr: u64) void {
        const va = va_split(virtual_addr);

        const pml4e = self.pml4.entries[va.pml4];
        if (pml4e & PF_PRESENT == 0) return;
        const pdpt: *RawTable = @alignCast(@ptrFromInt(pml4e & PHYS_ADDR_MASK));

        const pdpte = pdpt.entries[va.pdpt];
        if (pdpte & PF_PRESENT == 0) return;
        const pd: *RawTable = @alignCast(@ptrFromInt(pdpte & PHYS_ADDR_MASK));

        const pde = pd.entries[va.pd];
        if (pde & PF_PRESENT == 0) return;
        const pt: *RawTable = @alignCast(@ptrFromInt(pde & PHYS_ADDR_MASK));

        pt.entries[va.pt] = 0;
        invlpg(virtual_addr);
    }
};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Given a pointer to an entry in a parent table, ensure the child table
/// it points to exists (allocating one if necessary), and return a pointer
/// to the child table.
fn ensure_table(entry: *u64) !*RawTable {
    if (entry.* & PF_PRESENT != 0) {
        // Child table already exists — decode its address
        const addr = entry.* & PHYS_ADDR_MASK;
        return @alignCast(@ptrFromInt(addr));
    }
    // Allocate a new table page
    const raw = ppa.phys_alloc(1) orelse return error.OutOfMemory;
    const table: *RawTable = @alignCast(@ptrCast(raw));
    table.* = RawTable.zeroed();
    entry.* = @intFromPtr(&table.entries) | PF_PRESENT | PF_WRITABLE | PF_USER;
    return table;
}

/// Invalidate a single TLB entry.
inline fn invlpg(vaddr: u64) void {
    asm volatile ("invlpg (%[addr])"
        :
        : [addr] "r" (vaddr)
        : "memory"
    );
}

// ---------------------------------------------------------------------------
// Kernel global address space
// ---------------------------------------------------------------------------

var kernel_paging: ?PagingContext = null;
var paging_initialized: bool = false;

pub fn paging_global_init() void {
    if (paging_initialized) return;

    kernel_paging = PagingContext.create() orelse {
        vga.set_color(.LightRed, .Black);
        vga.print("[Paging] FATAL: cannot allocate PML4 — out of physical memory!\n");
        vga.set_color(.White, .Black);
        while (true) { asm volatile ("hlt"); }
    };

    // Activate the new kernel page table
    kernel_paging.?.activate();
    paging_initialized = true;

    vga.set_color(.Green, .Black);
    vga.print("[Paging] Kernel PML4 loaded into CR3 — 4-level paging active.\n");
    vga.set_color(.White, .Black);
}

// ---------------------------------------------------------------------------
// C-ABI exports (called from Rust via FFI)
// ---------------------------------------------------------------------------

/// Map a virtual page in the kernel address space.
/// flags: bit0=writable, bit1=user, bit2=no-execute
pub export fn page_table_map(virtual_addr: usize, physical_addr: usize, flags: u32) i32 {
    if (!paging_initialized) paging_global_init();
    var ctx = &(kernel_paging orelse return -1);

    var pte_flags: u64 = 0;
    if (flags & 1 != 0) pte_flags |= PF_WRITABLE;
    if (flags & 2 != 0) pte_flags |= PF_USER;
    if (flags & 4 == 0) pte_flags |= PF_NO_EXECUTE; // Executable only if bit2 set

    ctx.map(virtual_addr, physical_addr, pte_flags) catch return -1;
    return 0;
}

/// Unmap a virtual page from the kernel address space.
pub export fn page_table_unmap(virtual_addr: usize) void {
    if (!paging_initialized) paging_global_init();
    if (kernel_paging) |*ctx| {
        ctx.unmap(virtual_addr);
    }
}
