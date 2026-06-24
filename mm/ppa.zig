// mm/ppa.zig — Lofita OS Physical Page Allocator
// Written in Zig (freestanding — no std, no libc)
//
// Implements a buddy allocator over a statically reserved memory pool.
// Memory is divided into 4KB pages. Contiguous blocks of pages are
// managed in power-of-2 "orders" (order 0 = 1 page, order 10 = 1024 pages).
//
// On bare metal, the physical memory available to the kernel is described
// by the Multiboot2 memory map. For now we use a conservative static pool
// of 64MB placed in the BSS segment. In a later phase, ppa_init() will
// be extended to parse the Multiboot2 memory map and manage all available RAM.
//
// API (C-ABI exported for Rust FFI):
//   phys_alloc(pages: usize) -> ?[*]u8   allocate 'pages' contiguous pages
//   phys_free(ptr: ?[*]u8, pages: usize)  free previously allocated pages
//
// Zig-native API:
//   ppa_init()                            initialize the allocator (idempotent)

const vga = @import("../drivers/vga.zig");

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

pub const PAGE_SIZE:   usize = 4096;
pub const MAX_ORDER:   usize = 11;      // Maximum block = 2^10 = 1024 pages = 4MB
pub const MEMORY_SIZE: usize = 64 * 1024 * 1024; // 64 MB static pool
pub const TOTAL_PAGES: usize = MEMORY_SIZE / PAGE_SIZE;

// ---------------------------------------------------------------------------
// Physical memory pool
// Placed in .bss (zero-initialised at boot by the multiboot stub).
// Aligned to PAGE_SIZE so physical addresses == pointer values in identity map.
// ---------------------------------------------------------------------------

var mem_pool: [MEMORY_SIZE]u8 align(PAGE_SIZE) = undefined;

// ---------------------------------------------------------------------------
// Page metadata
// ---------------------------------------------------------------------------

const PageInfo = struct {
    order:   u6,
    is_free: bool,
};

var page_meta: [TOTAL_PAGES]PageInfo = undefined;
var initialized: bool = false;

// ---------------------------------------------------------------------------
// Spinlock (atomic test-and-set)
// ---------------------------------------------------------------------------

var ppa_lock: bool = false;

fn lock() void {
    while (@atomicRmw(bool, &ppa_lock, .Xchg, true, .acquire)) {
        asm volatile ("pause"); // Hint to CPU: this is a spin-wait loop
    }
}

fn unlock() void {
    @atomicStore(bool, &ppa_lock, false, .release);
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub fn ppa_init() void {
    if (initialized) return;
    lock();
    defer unlock();
    if (initialized) return; // Double-checked locking

    // Mark every page as free at order 0
    for (&page_meta) |*p| {
        p.* = PageInfo{ .order = 0, .is_free = true };
    }

    // Merge pages into maximum-order blocks
    const max_block: usize = 1 << (MAX_ORDER - 1);
    var i: usize = 0;
    while (i < TOTAL_PAGES) : (i += max_block) {
        page_meta[i].order = @intCast(MAX_ORDER - 1);
    }

    initialized = true;

    vga.set_color(.Green, .Black);
    vga.print("[PPA] Physical page allocator ready — ");
    vga.print_dec(MEMORY_SIZE / (1024 * 1024));
    vga.print(" MB pool at 0x");
    vga.print_hex(@intFromPtr(&mem_pool));
    vga.print("\n");
    vga.set_color(.White, .Black);
}

// ---------------------------------------------------------------------------
// Allocator — buddy split
// ---------------------------------------------------------------------------

pub export fn phys_alloc(pages: usize) callconv(.c) ?[*]u8 {
    if (!initialized) ppa_init();
    if (pages == 0) return null;

    lock();
    defer unlock();

    // Find the smallest order that fits 'pages' pages
    var req_order: u6 = 0;
    while ((@as(usize, 1) << req_order) < pages) {
        req_order += 1;
        if (req_order >= MAX_ORDER) return null;
    }

    // Walk orders from req_order up, looking for a free block to split
    var current_order = req_order;
    while (current_order < MAX_ORDER) : (current_order += 1) {
        const block_size = @as(usize, 1) << current_order;
        var i: usize = 0;
        while (i < TOTAL_PAGES) : (i += block_size) {
            if (page_meta[i].is_free and page_meta[i].order == current_order) {
                // Split block down to req_order, releasing buddy halves
                var co = current_order;
                while (co > req_order) {
                    co -= 1;
                    const half = @as(usize, 1) << co;
                    const buddy = i + half;
                    page_meta[i].order     = co;
                    page_meta[buddy].order  = co;
                    page_meta[buddy].is_free = true;
                }
                page_meta[i].is_free = false;
                return @ptrCast(&mem_pool[i * PAGE_SIZE]);
            }
        }
    }
    return null; // Out of memory
}

// ---------------------------------------------------------------------------
// Deallocator — buddy coalesce
// ---------------------------------------------------------------------------

pub export fn phys_free(ptr: ?[*]u8, _pages: usize) callconv(.c) void {
    _ = _pages; // Ignored — we recover size from page_meta

    if (!initialized or ptr == null) return;

    const addr      = @intFromPtr(ptr);
    const pool_base = @intFromPtr(&mem_pool);

    // Validate pointer is within our pool and page-aligned
    if (addr < pool_base or addr >= pool_base + MEMORY_SIZE) return;
    const offset = addr - pool_base;
    if (offset % PAGE_SIZE != 0) return;

    const start_page = offset / PAGE_SIZE;
    if (start_page >= TOTAL_PAGES) return;

    lock();
    defer unlock();

    if (page_meta[start_page].is_free) return; // Double-free guard

    var order = page_meta[start_page].order;
    var idx   = start_page;
    page_meta[idx].is_free = true;

    // Coalesce with buddy while possible
    while (order < MAX_ORDER - 1) {
        const block_size = @as(usize, 1) << order;
        const buddy_idx  = idx ^ block_size;
        if (buddy_idx >= TOTAL_PAGES) break;

        if (page_meta[buddy_idx].is_free and page_meta[buddy_idx].order == order) {
            page_meta[buddy_idx].is_free = false;
            idx = if (buddy_idx < idx) buddy_idx else idx;
            order += 1;
            page_meta[idx].order   = order;
            page_meta[idx].is_free = true;
        } else {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostics (called from kernel_main for boot-time reporting)
// ---------------------------------------------------------------------------

pub fn free_pages() usize {
    var count: usize = 0;
    var i: usize = 0;
    while (i < TOTAL_PAGES) : (i += 1) {
        if (page_meta[i].is_free) count += (@as(usize, 1) << page_meta[i].order);
    }
    return count;
}
