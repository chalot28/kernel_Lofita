// Lorifa Monolithic Kernel - Bootloader Initialization
// Written in Zig
// Boots segments, traps, and memory page allocators.

const std = @import("std");
const gdt = @import("../arch/x86_64/kernel/gdt.zig");
const idt = @import("../arch/x86_64/kernel/idt.zig");
const ppa = @import("../mm/ppa.zig");

// Rust Core Init
extern fn rust_kernel_init() void;

pub fn main() !void {
    std.debug.print("==================================================\n", .{});
    std.debug.print("      Booting Lorifa Monolithic Kernel (x86_64)   \n", .{});
    std.debug.print("==================================================\n", .{});

    // 1. GDT segment descriptors
    gdt.gdt_init();

    // 2. IDT trap registers
    idt.idt_init();

    // 3. Physical Page Allocator (PPA)
    ppa.ppa_init();

    // 4. Rust Core policies & TMD
    std.debug.print("[init] Low-level initializations complete. Invoking Rust Core...\n", .{});
    rust_kernel_init();

    std.debug.print("[init] Monolithic kernel loaded successfully.\n", .{});
}
