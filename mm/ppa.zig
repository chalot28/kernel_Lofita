// mm/ppa.zig
// Written in Zig
// Subsystem-independent physical memory page allocator.

const std = @import("std");

pub const PAGE_SIZE: usize = 4096;
pub const MAX_ORDER: usize = 11;
pub const MEMORY_SIZE: usize = 64 * 1024 * 1024;
pub const TOTAL_PAGES: usize = MEMORY_SIZE / PAGE_SIZE;

var mem_pool: [MEMORY_SIZE]u8 align(PAGE_SIZE) = undefined;

const PageInfo = struct {
    order: u8,
    is_free: bool,
};

var page_table: [TOTAL_PAGES]PageInfo = undefined;
var initialized: bool = false;

pub fn ppa_init() void {
    if (initialized) return;

    var i: usize = 0;
    while (i < TOTAL_PAGES) {
        page_table[i] = PageInfo{ .order = 0, .is_free = true };
        i += 1;
    }

    const max_block_pages = @as(usize, 1) << (MAX_ORDER - 1);
    i = 0;
    while (i < TOTAL_PAGES) : (i += max_block_pages) {
        page_table[i].order = @intCast(MAX_ORDER - 1);
    }

    initialized = true;
}

pub export fn phys_alloc(pages: usize) callconv(.C) ?[*]u8 {
    if (!initialized) ppa_init();
    if (pages == 0) return null;

    var req_order: u8 = 0;
    while ((@as(usize, 1) << req_order) < pages) {
        req_order += 1;
        if (req_order >= MAX_ORDER) return null;
    }

    var current_order = req_order;
    while (current_order < MAX_ORDER) : (current_order += 1) {
        const block_size = @as(usize, 1) << current_order;
        var i: usize = 0;
        while (i < TOTAL_PAGES) : (i += block_size) {
            if (page_table[i].is_free and page_table[i].order == current_order) {
                while (current_order > req_order) {
                    current_order -= 1;
                    const half_size = @as(usize, 1) << current_order;
                    const buddy_idx = i + half_size;
                    page_table[i].order = current_order;
                    page_table[buddy_idx].order = current_order;
                    page_table[buddy_idx].is_free = true;
                }
                page_table[i].is_free = false;
                return @ptrCast(&mem_pool[i * PAGE_SIZE]);
            }
        }
    }
    return null;
}

pub export fn phys_free(ptr: ?[*]u8, pages: usize) callconv(.C) void {
    if (!initialized or ptr == null) return;
    const addr = @intFromPtr(ptr);
    const pool_addr = @intFromPtr(&mem_pool);
    if (addr < pool_addr or addr >= pool_addr + MEMORY_SIZE) return;

    const offset = addr - pool_addr;
    if (offset % PAGE_SIZE != 0) return;

    const start_page = offset / PAGE_SIZE;
    if (start_page >= TOTAL_PAGES) return;

    var req_order: u8 = 0;
    while ((@as(usize, 1) << req_order) < pages) {
        req_order += 1;
    }

    var current_order = req_order;
    var page_idx = start_page;
    page_table[page_idx].is_free = true;

    while (current_order < MAX_ORDER - 1) {
        const block_size = @as(usize, 1) << current_order;
        const buddy_idx = page_idx ^ block_size;
        if (buddy_idx >= TOTAL_PAGES) break;

        if (page_table[buddy_idx].is_free and page_table[buddy_idx].order == current_order) {
            page_table[buddy_idx].is_free = false;
            if (buddy_idx < page_idx) {
                page_idx = buddy_idx;
            }
            current_order += 1;
            page_table[page_idx].order = current_order;
            page_table[page_idx].is_free = true;
        } else {
            break;
        }
    }
}
