// arch/x86_64/kernel/gdt.zig
// Written in Zig
// Global Descriptor Table segment rules.

const std = @import("std");

pub const GdtEntry = struct {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    flags: u8,
    base_high: u8,
};

pub const GdtPointer = struct {
    limit: u16,
    base: usize,
};

var gdt_entries: [5]GdtEntry = undefined;
var gdt_ptr: GdtPointer = undefined;

pub fn gdt_init() void {
    gdt_entries[0] = create_entry(0, 0, 0, 0);
    gdt_entries[1] = create_entry(0, 0xFFFFFFFF, 0x9A, 0x20); // Kernel Code
    gdt_entries[2] = create_entry(0, 0xFFFFFFFF, 0x92, 0x00); // Kernel Data
    gdt_entries[3] = create_entry(0, 0xFFFFFFFF, 0xFA, 0x20); // User Code
    gdt_entries[4] = create_entry(0, 0xFFFFFFFF, 0xF2, 0x00); // User Data

    gdt_ptr.limit = @intCast(gdt_entries.len * @sizeOf(GdtEntry) - 1);
    gdt_ptr.base = @intFromPtr(&gdt_entries);

    std.debug.print("[arch/x86_64/kernel/gdt] Load segments (Kernel + User Ring 3).\n", .{});
}

fn create_entry(base: u32, limit: u32, access: u8, flags: u8) GdtEntry {
    return GdtEntry{
        .limit_low = @intCast(limit & 0xFFFF),
        .base_low = @intCast(base & 0xFFFF),
        .base_middle = @intCast((base >> 16) & 0xFF),
        .access_byte = access,
        .flags = @intCast(((limit >> 16) & 0x0F) | (flags & 0xF0)),
        .base_high = @intCast((base >> 24) & 0xFF),
    };
}
