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

pub const TrapFrame = extern struct {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    error_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
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
        \\# Save general-purpose registers to form a TrapFrame
        \\pushq %rax
        \\pushq %rbx
        \\pushq %rcx
        \\pushq %rdx
        \\pushq %rsi
        \\pushq %rdi
        \\pushq %rbp
        \\pushq %r8
        \\pushq %r9
        \\pushq %r10
        \\pushq %r11
        \\pushq %r12
        \\pushq %r13
        \\pushq %r14
        \\pushq %r15
        \\
        \\# Clear direction flag for ABI compliance
        \\cld
        \\
        \\# Pass arguments to Zig handler:
        \\# 1st argument (RDI) = Pointer to TrapFrame (current stack pointer)
        \\movq %rsp, %rdi
        \\# 2nd argument (RSI) = Faulting address from CR2 register
        \\movq %cr2, %rsi
        \\
        \\# Align stack pointer to 16 bytes for System V ABI compliance.
        \\# TrapFrame size is 168 bytes. At this point RSP is at (OriginalRSP - 168), which is 8-byte aligned.
        \\# Subtracting 8 makes RSP 16-byte aligned before the callq instruction.
        \\subq $8, %rsp
        \\callq page_fault_zig_handler
        \\addq $8, %rsp
        \\
        \\# Restore general-purpose registers from TrapFrame
        \\popq %r15
        \\popq %r14
        \\popq %r13
        \\popq %r12
        \\popq %r11
        \\popq %r10
        \\popq %r9
        \\popq %r8
        \\popq %rbp
        \\popq %rdi
        \\popq %rsi
        \\popq %rdx
        \\popq %rcx
        \\popq %rbx
        \\popq %rax
        \\
        \\# Pop hardware error code and return
        \\addq $8, %rsp
        \\iretq
    );
}

pub fn page_fault_zig_handler(frame: *TrapFrame, fault_address: usize) callconv(.C) void {
    std.debug.print(
        \\[IDT PageFault] Interrupt caught.
        \\  Faulting Address (CR2): 0x{x}
        \\  Error Code: 0x{x}
        \\  RIP: 0x{x}, CS: 0x{x}, RFLAGS: 0x{x}
        \\  RSP: 0x{x}, SS: 0x{x}
        \\  RAX: 0x{x}, RBX: 0x{x}, RCX: 0x{x}, RDX: 0x{x}
        \\  RSI: 0x{x}, RDI: 0x{x}, RBP: 0x{x}
        \\  R8:  0x{x}, R9:  0x{x}, R10: 0x{x}, R11: 0x{x}
        \\  R12: 0x{x}, R13: 0x{x}, R14: 0x{x}, R15: 0x{x}
        \\
    , .{
        fault_address,
        frame.error_code,
        frame.rip, frame.cs, frame.rflags,
        frame.rsp, frame.ss,
        frame.rax, frame.rbx, frame.rcx, frame.rdx,
        frame.rsi, frame.rdi, frame.rbp,
        frame.r8, frame.r9, frame.r10, frame.r11,
        frame.r12, frame.r13, frame.r14, frame.r15,
    });
}

fn syscall_int80_handler() callconv(.Naked) void {
    asm volatile (
        \\# Push dummy error code to make stack frame layout uniform with TrapFrame
        \\pushq $0
        \\
        \\# Save general-purpose registers
        \\pushq %rax
        \\pushq %rbx
        \\pushq %rcx
        \\pushq %rdx
        \\pushq %rsi
        \\pushq %rdi
        \\pushq %rbp
        \\pushq %r8
        \\pushq %r9
        \\pushq %r10
        \\pushq %r11
        \\pushq %r12
        \\pushq %r13
        \\pushq %r14
        \\pushq %r15
        \\
        \\# Clear direction flag for ABI compliance
        \\cld
        \\
        \\# Pass arguments to Zig handler:
        \\# 1st argument (RDI) = Pointer to TrapFrame (current stack pointer)
        \\movq %rsp, %rdi
        \\
        \\# Align stack pointer to 16 bytes for System V ABI compliance.
        \\# TrapFrame size is 168 bytes. At this point RSP is at (OriginalRSP - 168), which is 8-byte aligned.
        \\# Subtracting 8 makes RSP 16-byte aligned before the callq instruction.
        \\subq $8, %rsp
        \\callq syscall_int80_zig_handler
        \\addq $8, %rsp
        \\
        \\# Restore general-purpose registers from TrapFrame
        \\popq %r15
        \\popq %r14
        \\popq %r13
        \\popq %r12
        \\popq %r11
        \\popq %r10
        \\popq %r9
        \\popq %r8
        \\popq %rbp
        \\popq %rdi
        \\popq %rsi
        \\popq %rdx
        \\popq %rcx
        \\popq %rbx
        \\popq %rax
        \\
        \\# Pop dummy error code and return
        \\addq $8, %rsp
        \\iretq
    );
}

pub fn syscall_int80_zig_handler(frame: *TrapFrame) callconv(.C) void {
    std.debug.print(
        \\[IDT Syscall] int 0x80 syscall gate.
        \\  Syscall Number (RAX): {}
        \\  Arg1 (RDI): 0x{x}
        \\  Arg2 (RSI): 0x{x}
        \\  Arg3 (RDX): 0x{x}
        \\  Arg4 (R10): 0x{x}
        \\  Arg5 (R8):  0x{x}
        \\  Arg6 (R9):  0x{x}
        \\  RIP: 0x{x}, RSP: 0x{x}
        \\
    , .{
        frame.rax,
        frame.rdi,
        frame.rsi,
        frame.rdx,
        frame.r10,
        frame.r8,
        frame.r9,
        frame.rip,
        frame.rsp,
    });
    // Set RAX return value to 0 (Success) in the saved context
    frame.rax = 0;
}
