// arch/x86_64/kernel/idt.zig — Lofita OS Interrupt Descriptor Table
// Written in Zig (freestanding — no std, no libc)
//
// The IDT maps interrupt/exception vectors (0-255) to handler stubs.
// Each 16-byte gate descriptor encodes:
//   - Handler address (split: low 16b / middle 16b / high 32b)
//   - Code segment selector (0x08 = kernel code)
//   - IST index (0 = use current stack — no IST for now)
//   - Type + attributes byte
//
// Key vectors configured here:
//   #0   Division by Zero
//   #6   Invalid Opcode
//   #8   Double Fault        (critical — must never be unhandled)
//   #13  General Protection Fault
//   #14  Page Fault
//   #32+ Timer + future IRQs (after PIC init)
//   0x80 Linux int 0x80 syscall compatibility gate

const vga  = @import("../../drivers/vga.zig");
const gdt  = @import("gdt.zig");

// ---------------------------------------------------------------------------
// IDT entry (16 bytes, packed)
// ---------------------------------------------------------------------------

pub const IdtEntry = packed struct {
    offset_low:    u16,
    selector:      u16,
    ist:           u8,       // Interrupt Stack Table index (0 = no IST)
    type_attr:     u8,       // Gate type + DPL + Present
    offset_middle: u16,
    offset_high:   u32,
    reserved:      u32,
};

pub const IdtPointer = packed struct {
    limit: u16,
    base:  u64,
};

// ---------------------------------------------------------------------------
// Saved CPU state at interrupt time (matches the stack layout built by stubs)
// ---------------------------------------------------------------------------

pub const TrapFrame = extern struct {
    // Pushed by our stub (in reverse order of push):
    r15: u64, r14: u64, r13: u64, r12: u64,
    r11: u64, r10: u64, r9:  u64, r8:  u64,
    rbp: u64, rdi: u64, rsi: u64, rdx: u64,
    rcx: u64, rbx: u64, rax: u64,
    // Pushed by CPU (hardware error code — or dummy 0 for non-error exceptions):
    error_code: u64,
    // Pushed by CPU:
    rip: u64, cs: u64, rflags: u64, rsp: u64, ss: u64,
};

// ---------------------------------------------------------------------------
// IDT storage
// ---------------------------------------------------------------------------

var idt_entries: [256]IdtEntry align(16) = undefined;
var idt_ptr: IdtPointer = undefined;

// ---------------------------------------------------------------------------
// Entry construction
// ---------------------------------------------------------------------------

/// Gate type_attr values:
///   0x8E = Present | DPL=0 | 64-bit Interrupt Gate (disables interrupts on entry)
///   0x8F = Present | DPL=0 | 64-bit Trap Gate     (interrupts stay enabled)
///   0xEE = Present | DPL=3 | 64-bit Trap Gate     (userspace-callable, e.g. int 0x80)
fn make_gate(handler: usize, selector: u16, ist: u8, type_attr: u8) IdtEntry {
    return IdtEntry{
        .offset_low    = @intCast(handler & 0xFFFF),
        .selector      = selector,
        .ist           = ist,
        .type_attr     = type_attr,
        .offset_middle = @intCast((handler >> 16) & 0xFFFF),
        .offset_high   = @intCast((handler >> 32) & 0xFFFFFFFF),
        .reserved      = 0,
    };
}

// ---------------------------------------------------------------------------
// Macro-like helper: generate a naked ISR stub that saves all GPRs,
// optionally pushes a dummy error code, calls the Zig handler, restores
// GPRs, then iretq.
//
// Zig does not have C-style macros; instead we use comptime functions.
// The stub is generated for each vector at comptime.
// ---------------------------------------------------------------------------

/// Build a naked ISR stub for an exception that does NOT push an error code.
/// The stub pushes a dummy 0 to keep the TrapFrame layout uniform.
fn isr_stub_no_err(comptime handler_fn: anytype) fn() callconv(.Naked) void {
    return struct {
        fn stub() callconv(.Naked) void {
            asm volatile (
                \\ pushq $0          /* dummy error code */
                \\ pushq %rax
                \\ pushq %rbx
                \\ pushq %rcx
                \\ pushq %rdx
                \\ pushq %rsi
                \\ pushq %rdi
                \\ pushq %rbp
                \\ pushq %r8
                \\ pushq %r9
                \\ pushq %r10
                \\ pushq %r11
                \\ pushq %r12
                \\ pushq %r13
                \\ pushq %r14
                \\ pushq %r15
                \\ cld
                \\ movq  %rsp, %rdi  /* arg0 = *TrapFrame */
                \\ subq  $8, %rsp    /* 16-byte stack alignment */
                \\ callq %[fn]
                \\ addq  $8, %rsp
                \\ popq  %r15
                \\ popq  %r14
                \\ popq  %r13
                \\ popq  %r12
                \\ popq  %r11
                \\ popq  %r10
                \\ popq  %r9
                \\ popq  %r8
                \\ popq  %rbp
                \\ popq  %rdi
                \\ popq  %rsi
                \\ popq  %rdx
                \\ popq  %rcx
                \\ popq  %rbx
                \\ popq  %rax
                \\ addq  $8, %rsp    /* discard dummy error code */
                \\ iretq
                :
                : [fn] "i" (&handler_fn)
                : "memory"
            );
        }
    }.stub;
}

/// Build a naked ISR stub for an exception that DOES push an error code (CPU-pushed).
fn isr_stub_with_err(comptime handler_fn: anytype) fn() callconv(.Naked) void {
    return struct {
        fn stub() callconv(.Naked) void {
            asm volatile (
                \\ /* error code already on stack from CPU */
                \\ pushq %rax
                \\ pushq %rbx
                \\ pushq %rcx
                \\ pushq %rdx
                \\ pushq %rsi
                \\ pushq %rdi
                \\ pushq %rbp
                \\ pushq %r8
                \\ pushq %r9
                \\ pushq %r10
                \\ pushq %r11
                \\ pushq %r12
                \\ pushq %r13
                \\ pushq %r14
                \\ pushq %r15
                \\ cld
                \\ movq  %rsp, %rdi  /* arg0 = *TrapFrame */
                \\ subq  $8, %rsp
                \\ callq %[fn]
                \\ addq  $8, %rsp
                \\ popq  %r15
                \\ popq  %r14
                \\ popq  %r13
                \\ popq  %r12
                \\ popq  %r11
                \\ popq  %r10
                \\ popq  %r9
                \\ popq  %r8
                \\ popq  %rbp
                \\ popq  %rdi
                \\ popq  %rsi
                \\ popq  %rdx
                \\ popq  %rcx
                \\ popq  %rbx
                \\ popq  %rax
                \\ addq  $8, %rsp    /* discard CPU-pushed error code */
                \\ iretq
                :
                : [fn] "i" (&handler_fn)
                : "memory"
            );
        }
    }.stub;
}

// ---------------------------------------------------------------------------
// Exception handler implementations
// ---------------------------------------------------------------------------

/// #DE — Division by Zero (vector 0, no error code)
pub fn handle_div_zero(frame: *TrapFrame) callconv(.C) void {
    vga.set_color(.LightRed, .Black);
    vga.print("[EXCEPTION #DE] Division by Zero!\n");
    vga.print("  RIP="); vga.print_hex(frame.rip);
    vga.print(" CS=");   vga.print_hex(frame.cs);
    vga.print("\n");
    vga.set_color(.White, .Black);
    // Halt — cannot recover from this without fixing RIP
    while (true) { asm volatile ("hlt"); }
}

/// #UD — Invalid Opcode (vector 6, no error code)
pub fn handle_invalid_opcode(frame: *TrapFrame) callconv(.C) void {
    vga.set_color(.LightRed, .Black);
    vga.print("[EXCEPTION #UD] Invalid Opcode at RIP=");
    vga.print_hex(frame.rip);
    vga.print("\n");
    vga.set_color(.White, .Black);
    while (true) { asm volatile ("hlt"); }
}

/// #DF — Double Fault (vector 8, error code always 0)
pub fn handle_double_fault(frame: *TrapFrame) callconv(.C) void {
    vga.set_color(.White, .Red);
    vga.print("\n*** KERNEL PANIC: Double Fault ***\n");
    vga.print("RIP="); vga.print_hex(frame.rip);
    vga.print(" RSP="); vga.print_hex(frame.rsp);
    vga.print("\n");
    while (true) { asm volatile ("cli\nhlt"); }
}

/// #GP — General Protection Fault (vector 13, has error code)
pub fn handle_gpf(frame: *TrapFrame) callconv(.C) void {
    vga.set_color(.LightRed, .Black);
    vga.print("[EXCEPTION #GP] General Protection Fault\n");
    vga.print("  Error code="); vga.print_hex(frame.error_code);
    vga.print(" RIP=");         vga.print_hex(frame.rip);
    vga.print("\n");
    vga.set_color(.White, .Black);
    while (true) { asm volatile ("hlt"); }
}

/// #PF — Page Fault (vector 14, has error code = access flags)
pub fn handle_page_fault(frame: *TrapFrame) callconv(.C) void {
    var cr2: u64 = 0;
    asm volatile ("movq %%cr2, %[out]"
        : [out] "=r" (cr2)
    );
    vga.set_color(.Yellow, .Black);
    vga.print("[EXCEPTION #PF] Page Fault\n");
    vga.print("  Faulting addr (CR2)="); vga.print_hex(cr2);
    vga.print("\n  Error code=");         vga.print_hex(frame.error_code);
    // Decode error bits
    if (frame.error_code & 1 != 0) { vga.print(" [Protection]"); }
    else                            { vga.print(" [Not-Present]"); }
    if (frame.error_code & 2 != 0) { vga.print(" [Write]"); }
    else                            { vga.print(" [Read]");  }
    if (frame.error_code & 4 != 0) { vga.print(" [User]");   }
    else                            { vga.print(" [Kernel]"); }
    vga.print("\n  RIP="); vga.print_hex(frame.rip);
    vga.print(" RSP=");    vga.print_hex(frame.rsp);
    vga.print("\n");
    vga.set_color(.White, .Black);
    // TODO: in Phase 3 this will trigger ELF demand-paging
    while (true) { asm volatile ("hlt"); }
}

/// int 0x80 — Linux syscall compatibility gate (vector 128, no error code)
pub fn handle_syscall_int80(frame: *TrapFrame) callconv(.C) void {
    // Syscall number in RAX; args in RDI, RSI, RDX, R10, R8, R9
    // (Linux x86_64 ABI for int 0x80 on 64-bit uses the same registers
    //  as the SYSCALL instruction.)
    //
    // Dispatch to the Rust compatibility layer.
    // For now: route through the extern C function exported by Rust.
    const result = rust_syscall_dispatch(
        frame.rax,   // syscall number
        frame.rdi,   // arg1
        frame.rsi,   // arg2
        frame.rdx,   // arg3
        frame.r10,   // arg4
        frame.r8,    // arg5
        frame.r9,    // arg6
    );
    frame.rax = @bitCast(result); // Return value in RAX
}

/// Extern Rust function: routes syscall numbers to internal Lofita handlers.
extern fn rust_syscall_dispatch(
    nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64,
) i64;

// ---------------------------------------------------------------------------
// Stub instances
// ---------------------------------------------------------------------------

const stub_div_zero        = isr_stub_no_err(handle_div_zero);
const stub_invalid_opcode  = isr_stub_no_err(handle_invalid_opcode);
const stub_double_fault    = isr_stub_with_err(handle_double_fault);
const stub_gpf             = isr_stub_with_err(handle_gpf);
const stub_page_fault      = isr_stub_with_err(handle_page_fault);
const stub_syscall_int80   = isr_stub_no_err(handle_syscall_int80);

// ---------------------------------------------------------------------------
// Public init
// ---------------------------------------------------------------------------

pub fn idt_init() void {
    // Zero-fill all entries (marks them as not-present)
    for (&idt_entries) |*e| {
        e.* = make_gate(0, gdt.KERNEL_CODE_SEL, 0, 0x00);
    }

    // --- CPU Exceptions ---
    idt_entries[0]  = make_gate(@intFromPtr(&stub_div_zero),       gdt.KERNEL_CODE_SEL, 0, 0x8E); // #DE
    idt_entries[6]  = make_gate(@intFromPtr(&stub_invalid_opcode), gdt.KERNEL_CODE_SEL, 0, 0x8E); // #UD
    idt_entries[8]  = make_gate(@intFromPtr(&stub_double_fault),   gdt.KERNEL_CODE_SEL, 0, 0x8E); // #DF
    idt_entries[13] = make_gate(@intFromPtr(&stub_gpf),            gdt.KERNEL_CODE_SEL, 0, 0x8E); // #GP
    idt_entries[14] = make_gate(@intFromPtr(&stub_page_fault),     gdt.KERNEL_CODE_SEL, 0, 0x8E); // #PF

    // --- Linux syscall compatibility gate (DPL=3 so userspace can call) ---
    idt_entries[128] = make_gate(@intFromPtr(&stub_syscall_int80), gdt.KERNEL_CODE_SEL, 0, 0xEE);

    idt_ptr.limit = @intCast(idt_entries.len * @sizeOf(IdtEntry) - 1);
    idt_ptr.base  = @intFromPtr(&idt_entries);

    // --- Load the IDT into the CPU ---
    asm volatile ("lidt (%[ptr])"
        :
        : [ptr] "r" (&idt_ptr)
        : "memory"
    );

    // --- Enable hardware interrupts ---
    // (IRQ 0 = PIT timer will fire once we configure the PIC in a later phase)
    asm volatile ("sti");

    vga.set_color(.Green, .Black);
    vga.print("[IDT] Loaded — exceptions + int 0x80 syscall gate active.\n");
    vga.set_color(.White, .Black);
}
