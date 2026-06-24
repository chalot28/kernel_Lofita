// arch/x86_64/kernel/gdt.zig — Lofita OS Global Descriptor Table
// Written in Zig (freestanding — no std, no libc)
//
// The GDT defines memory segment descriptors that control privilege levels,
// code/data access, and 64-bit mode flags for the CPU.
//
// Segment layout:
//   0x00 — Null descriptor (required by CPU)
//   0x08 — Kernel Code segment (DPL=0, 64-bit, L=1)
//   0x10 — Kernel Data segment (DPL=0)
//   0x18 — User Code segment   (DPL=3, 64-bit, L=1)
//   0x20 — User Data segment   (DPL=3)
//   0x28 — TSS low  (16-byte descriptor — for future Task State Segment)
//   0x30 — TSS high

const vga = @import("../../../drivers/vga.zig");

// ---------------------------------------------------------------------------
// Data structures (packed for correct binary layout)
// ---------------------------------------------------------------------------

pub const GdtEntry = packed struct {
    limit_low:   u16,
    base_low:    u16,
    base_middle: u8,
    access_byte: u8,
    /// High nibble: flags (G, DB, L, AVL); Low nibble: limit_high
    flags_limit_high: u8,
    base_high:   u8,
};

pub const GdtPointer = packed struct {
    limit: u16,
    base:  u64,
};

/// 16-byte TSS descriptor stored as two consecutive GdtEntry slots.
/// We reserve space now; the TSS itself will be filled in when we add
/// interrupt stacks (IST) in a later phase.
pub const TssDescriptor = packed struct {
    low:  GdtEntry,
    high: GdtEntry,
};

// ---------------------------------------------------------------------------
// GDT storage (must be static — CPU reads it after lgdt)
// ---------------------------------------------------------------------------

var gdt_entries: [7]GdtEntry align(8) = undefined;
var gdt_ptr: GdtPointer = undefined;

// ---------------------------------------------------------------------------
// Entry construction helpers
// ---------------------------------------------------------------------------

/// Build a standard 8-byte segment descriptor.
///
/// access byte bits (from MSB):
///   7   Present
///   6-5 DPL (privilege level: 0=kernel, 3=user)
///   4   Descriptor type (1=code/data, 0=system)
///   3   Executable
///   2   Direction/Conforming
///   1   Read/Write
///   0   Accessed (CPU sets this)
///
/// flags nibble (upper 4 bits of byte 6):
///   3   Granularity (G): 1=4KB page granularity, 0=byte
///   2   Size (DB):       0 in 64-bit mode
///   1   Long mode (L):   1 for 64-bit code segment
///   0   AVL
fn make_entry(base: u32, limit: u32, access: u8, flags: u8) GdtEntry {
    return GdtEntry{
        .limit_low         = @intCast(limit & 0xFFFF),
        .base_low          = @intCast(base  & 0xFFFF),
        .base_middle       = @intCast((base  >> 16) & 0xFF),
        .access_byte       = access,
        .flags_limit_high  = @intCast(((limit >> 16) & 0x0F) | (flags & 0xF0)),
        .base_high         = @intCast((base  >> 24) & 0xFF),
    };
}

// ---------------------------------------------------------------------------
// Public init
// ---------------------------------------------------------------------------

pub fn gdt_init() void {
    // --- Descriptor definitions ---
    // access byte: 0x9A = Present | DPL=0 | Type=code | Executable | Readable
    // flags:       0x20 = L=1 (64-bit code)
    gdt_entries[0] = make_entry(0, 0,          0x00, 0x00); // Null
    gdt_entries[1] = make_entry(0, 0xFFFFFFFF, 0x9A, 0x20); // Kernel Code (64-bit)
    gdt_entries[2] = make_entry(0, 0xFFFFFFFF, 0x92, 0x00); // Kernel Data
    gdt_entries[3] = make_entry(0, 0xFFFFFFFF, 0xFA, 0x20); // User Code (64-bit)
    gdt_entries[4] = make_entry(0, 0xFFFFFFFF, 0xF2, 0x00); // User Data
    // Entries [5] and [6] reserved for TSS descriptor (future)
    gdt_entries[5] = make_entry(0, 0, 0x00, 0x00); // TSS low  (placeholder)
    gdt_entries[6] = make_entry(0, 0, 0x00, 0x00); // TSS high (placeholder)

    gdt_ptr.limit = @intCast(gdt_entries.len * @sizeOf(GdtEntry) - 1);
    gdt_ptr.base  = @intFromPtr(&gdt_entries);

    // --- Load the GDT into the CPU ---
    // `lgdt` takes a 10-byte memory operand: 2-byte limit + 8-byte base.
    asm volatile ("lgdt (%[ptr])"
        :
        : [ptr] "r" (&gdt_ptr)
        : .{ .memory = true }
    );

    // --- Reload segment registers ---
    // After lgdt the cached segment descriptors are stale.
    // We use a far return to reload CS (code segment) with 0x08 (kernel code).
    // Other data segment registers are reloaded explicitly with 0x10.
    asm volatile (
        \\ /* Push new CS and the address of 1f onto the stack, then retfq */
        \\ push $0x08
        \\ lea  1f(%rip), %rax
        \\ push %rax
        \\ lretq
        \\ 1:
        \\ /* Reload data segment registers */
        \\ mov $0x10, %ax
        \\ mov %ax, %ds
        \\ mov %ax, %es
        \\ mov %ax, %fs
        \\ mov %ax, %gs
        \\ mov %ax, %ss
        :
        :
        : .{ .rax = true, .memory = true }
    );

    vga.set_color(.Green, .Black);
    vga.print("[GDT] Loaded — Kernel/User segments + 64-bit long mode active.\n");
    vga.set_color(.White, .Black);
}

// ---------------------------------------------------------------------------
// Selector constants (for use by IDT, TSS, syscall setup)
// ---------------------------------------------------------------------------
pub const KERNEL_CODE_SEL: u16 = 0x08;
pub const KERNEL_DATA_SEL: u16 = 0x10;
pub const USER_CODE_SEL:   u16 = 0x18 | 3; // RPL=3
pub const USER_DATA_SEL:   u16 = 0x20 | 3; // RPL=3
