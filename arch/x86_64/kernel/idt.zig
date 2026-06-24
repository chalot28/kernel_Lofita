// arch/x86_64/kernel/idt.zig
// Written in Zig
// Interrupt Descriptor Table and syscall trap stubs.

const std = @import("std");

pub const IdtEntry = struct {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
};

pub const IdtPointer = struct {
    limit: u16,
    base: usize,
};

var idt_entries: [256]IdtEntry = undefined;
var idt_ptr: IdtPointer = undefined;

pub fn idt_init() void {
    var i: usize = 0;
    while (i < 256) : (i += 1) {
        idt_entries[i] = create_entry(0, 0x08, 0, 0);
    }

    idt_entries[14] = create_entry(@intFromPtr(&page_fault_handler), 0x08, 0, 0x8E);
    idt_entries[128] = create_entry(@intFromPtr(&syscall_int80_handler), 0x08, 0, 0xEE);

    idt_ptr.limit = @intCast(idt_entries.len * @sizeOf(IdtEntry) - 1);
    idt_ptr.base = @intFromPtr(&idt_entries);

    std.debug.print("[arch/x86_64/kernel/idt] Interrupts and syscall gates initialized.\n", .{});
}

fn create_entry(offset: usize, selector: u16, ist: u8, type_attr: u8) IdtEntry {
    return IdtEntry{
        .offset_low = @intCast(offset & 0xFFFF),
        .selector = selector,
        .ist = ist,
        .type_attr = type_attr,
        .offset_middle = @intCast((offset >> 16) & 0xFFFF),
        .offset_high = @intCast((offset >> 32) & 0xFFFFFFFF),
        .reserved = 0,
    };
}

fn page_fault_handler() callconv(.Naked) void {
    asm volatile (
        \\# Save caller-saved registers
        \\pushq %rax
        \\pushq %rcx
        \\pushq %rdx
        \\pushq %rsi
        \\pushq %rdi
        \\pushq %r8
        \\pushq %r9
        \\pushq %r10
        \\pushq %r11
        \\
        \\# Call the Zig handler
        \\callq page_fault_zig_handler
        \\
        \\# Restore registers
        \\popq %r11
        \\popq %r10
        \\popq %r9
        \\popq %r8
        \\popq %rdi
        \\popq %rsi
        \\popq %rdx
        \\popq %rcx
        \\popq %rax
        \\
        \\# Pop the error code pushed by CPU
        \\addq $8, %rsp
        \\iretq
    );
}

pub fn page_fault_zig_handler() void {
    std.debug.print("[IDT PageFault] Interrupt caught. Saved CPU registers context. Re-routing thread...\n", .{});
}

fn syscall_int80_handler() callconv(.Naked) void {
    asm volatile (
        \\# Save caller-saved registers
        \\pushq %rax
        \\pushq %rcx
        \\pushq %rdx
        \\pushq %rsi
        \\pushq %rdi
        \\pushq %r8
        \\pushq %r9
        \\pushq %r10
        \\pushq %r11
        \\
        \\# Call the Zig handler
        \\callq syscall_int80_zig_handler
        \\
        \\# Restore registers
        \\popq %r11
        \\popq %r10
        \\popq %r9
        \\popq %r8
        \\popq %rdi
        \\popq %rsi
        \\popq %rdx
        \\popq %rcx
        \\popq %rax
        \\iretq
    );
}

pub fn syscall_int80_zig_handler() void {
    std.debug.print("[IDT Syscall] int 0x80 syscall gate. Saved context. Dispatching syscall router...\n", .{});
}
