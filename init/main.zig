// init/main.zig — Lofita OS Kernel Main Entry Point
// Written in Zig (freestanding — no std, no libc)
//
// kernel_main() is called by the Multiboot2 boot stub (_start64 in
// boot/multiboot.zig) after:
//   - Long mode is active
//   - BSS is zeroed
//   - A 32KB boot stack is set up
//   - Interrupts are disabled
//
// This function must NEVER return. If it does, the boot stub halts the CPU.

// Pull in the boot stub so its .multiboot section and _start symbol are
// included in the final binary. Without this, the linker will not include
// the Multiboot2 header and GRUB will refuse to load the kernel.
comptime {
    _ = @import("../boot/multiboot.zig");
}

const vga    = @import("../drivers/vga.zig");
const gdt    = @import("../arch/x86_64/kernel/gdt.zig");
const idt    = @import("../arch/x86_64/kernel/idt.zig");
const ppa    = @import("../mm/ppa.zig");
const paging = @import("../arch/x86_64/mm/paging.zig");

/// Called by Rust after all Rust-side kernel subsystems are initialized.
/// Declared here so the Rust crate can call it via FFI.
extern fn rust_kernel_init() void;

// ---------------------------------------------------------------------------
// Kernel entry point
// ---------------------------------------------------------------------------

/// kernel_main — called from the 64-bit boot stub.
/// No return type: this function runs forever (or panics/halts).
pub export fn kernel_main() noreturn {

    // -----------------------------------------------------------------------
    // 1. VGA driver — must be first so all subsequent steps can print
    // -----------------------------------------------------------------------
    vga.init();

    vga.set_color(.Cyan, .Black);
    vga.print("==================================================\n");
    vga.print("      Lofita Monolithic Kernel (x86_64)           \n");
    vga.print("      Bare-Metal Boot — Phase 1                   \n");
    vga.print("==================================================\n");
    vga.set_color(.White, .Black);

    // -----------------------------------------------------------------------
    // 2. Global Descriptor Table — load real segment descriptors
    // -----------------------------------------------------------------------
    vga.print("[init] Initializing GDT...\n");
    gdt.gdt_init();

    // -----------------------------------------------------------------------
    // 3. Interrupt Descriptor Table — register exception + syscall handlers
    // -----------------------------------------------------------------------
    vga.print("[init] Initializing IDT...\n");
    idt.idt_init();

    // -----------------------------------------------------------------------
    // 4. Physical Page Allocator — buddy allocator over 64MB static pool
    // -----------------------------------------------------------------------
    vga.print("[init] Initializing Physical Page Allocator...\n");
    ppa.ppa_init();

    // -----------------------------------------------------------------------
    // 5. Paging — set up kernel PML4 and load into CR3
    // -----------------------------------------------------------------------
    vga.print("[init] Initializing 4-level page tables...\n");
    paging.paging_global_init();

    // -----------------------------------------------------------------------
    // 6. Rust Core — initialize kernel policies, token system, VFS, IPC
    // -----------------------------------------------------------------------
    vga.print("[init] Invoking Rust core subsystems...\n");
    rust_kernel_init();

    // -----------------------------------------------------------------------
    // Boot complete banner
    // -----------------------------------------------------------------------
    vga.set_color(.LightGreen, .Black);
    vga.print("\n[init] *** Lofita Kernel loaded successfully. ***\n");
    vga.print("[init] CPU halting in idle loop. Press any key in QEMU.\n");
    vga.set_color(.White, .Black);

    // -----------------------------------------------------------------------
    // Idle loop — a real kernel would context-switch here.
    // In Phase 2 this will dispatch to the scheduler.
    // -----------------------------------------------------------------------
    while (true) {
        asm volatile ("hlt"); // Sleep until next interrupt
    }
}
