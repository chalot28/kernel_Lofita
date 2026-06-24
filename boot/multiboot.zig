// boot/multiboot.zig — Lofita OS Multiboot2 Boot Stub
// Written in Zig (freestanding — no std, no libc)
//
// This file provides:
//   1. A valid Multiboot2 header so GRUB can identify and load the kernel.
//   2. The _start entry point that GRUB jumps to in 32-bit protected mode.
//   3. Stack setup and BSS zeroing before calling kernel_main().
//
// GRUB contract on entry to _start:
//   EAX = 0x36D76289  (Multiboot2 magic)
//   EBX = physical address of Multiboot2 information structure
//   CS  = 32-bit flat code segment (0x08)
//   All other segment registers = 32-bit flat data segment (0x10)
//   Interrupts disabled (IF=0)
//   Paging disabled (CR0.PG=0)
//   CPU is in 32-bit protected mode
//
// We must then transition to 64-bit long mode ourselves before calling
// Zig kernel code compiled for x86_64.

// ---------------------------------------------------------------------------
// Multiboot2 Header (Section: .multiboot, must be 8-byte aligned)
// ---------------------------------------------------------------------------
//
// The spec (https://www.gnu.org/software/grub/manual/multiboot2) requires:
//   u32 magic       = 0xE85250D6
//   u32 architecture = 0 (i386/x86 protected mode)
//   u32 header_length = size of the whole header
//   u32 checksum     = -(magic + arch + header_length)
// followed by tag structures terminated by an end tag.
//
// NOTE: No framebuffer tag is included — we omit it so GRUB stays in
// legacy VGA text mode and the VGA buffer at 0xB8000 works.

comptime {
    // Force this section to be included in the output even if unreferenced.
    // The linker script places .multiboot at the very start of the file.
    asm (
        \\ .section .multiboot, "a"
        \\ .align 8
        \\
        \\ multiboot2_header_start:
        \\   .long 0xE85250D6          /* magic */
        \\   .long 0                   /* architecture: i386 protected mode */
        \\   .long (multiboot2_header_end - multiboot2_header_start)
        \\   .long -(0xE85250D6 + 0 + (multiboot2_header_end - multiboot2_header_start))
        \\
        \\   /* --- Information request tag (type=1): ask GRUB for memory map --- */
        \\   .align 8
        \\   .short 1                  /* tag type: information request */
        \\   .short 0                  /* flags */
        \\   .long  12                 /* size */
        \\   .long  6                  /* MBI type 6 = memory map */
        \\
        \\   /* --- End tag --- */
        \\   .align 8
        \\   .short 0                  /* type  = 0 (end) */
        \\   .short 0                  /* flags = 0 */
        \\   .long  8                  /* size  = 8 */
        \\ multiboot2_header_end:
    );

    // ---------------------------------------------------------------------------
    // 32-bit bootstrap: _start (entered from GRUB in 32-bit protected mode)
    //
    // Responsibilities:
    //   1. Save Multiboot2 info pointer (EBX) for later use.
    //   2. Set up a temporary 32-bit stack.
    //   3. Enable SSE (required by Zig-generated code).
    //   4. Set up minimal 64-bit page tables (identity map first 2GB).
    //   5. Enable PAE, then long mode (EFER.LME), then paging (CR0.PG).
    //   6. Load a 64-bit GDT.
    //   7. Far-jump into 64-bit code segment → _start64.
    // ---------------------------------------------------------------------------
    asm (
        \\ .section .text
        \\ .code32
        \\ .global _start
        \\ _start:
        \\   /* Disable interrupts and clear direction flag */
        \\   cli
        \\   cld
        \\
        \\   /* DEBUG: write white 'L' to VGA from 32-bit mode via register */
        \\   mov $0xB8000, %edi
        \\   movw $0x0F4C, (%edi)
        \\
        \\   /* Save multiboot2 info ptr (EBX) and magic (EAX) via register */
        \\   mov $mb2_info_ptr, %edi
        \\   mov %ebx, (%edi)
        \\   mov $mb2_magic, %edi
        \\   mov %eax, (%edi)
        \\
        \\   /* Set up temporary 32-bit stack (grows down from label) */
        \\   mov $_boot_stack_top, %esp
        \\
        \\   /* ---------------------------------------------------------------- */
        \\   /* Enable SSE so Zig can use XMM registers                         */
        \\   /* ---------------------------------------------------------------- */
        \\   mov %cr0, %eax
        \\   and $0xFFFFFFFB, %eax    /* Clear CR0.EM (no x87 emulation) */
        \\   or  $0x00000002, %eax    /* Set  CR0.MP (monitor co-processor) */
        \\   mov %eax, %cr0
        \\   mov %cr4, %eax
        \\   or  $0x00000600, %eax    /* Set CR4.OSFXSR and CR4.OSXMMEXCPT */
        \\   mov %eax, %cr4
        \\
        \\   /* ---------------------------------------------------------------- */
        \\   /* Build identity-mapped page tables for the first 2 GB            */
        \\   /* ---------------------------------------------------------------- */
        \\   /* Zero out page table area (3 * 4096 bytes) */
        \\   mov $_pml4, %edi
        \\   xor %eax, %eax
        \\   mov $3072, %ecx          /* 3 pages * 1024 dwords */
        \\   rep stosl
        \\
        \\   /* PML4[0] = &PDPT | PRESENT | WRITABLE */
        \\   mov $_pml4,  %edi
        \\   mov $_pdpt,  %eax
        \\   or  $0x3, %eax
        \\   mov %eax, (%edi)
        \\
        \\   /* Map first 2GB via 1GB huge pages in PDPT */
        \\   mov $_pdpt, %edi
        \\   /* PDPT[0] = 0x00000000 | PRESENT | WRITABLE | HUGE (1GB page) */
        \\   mov $0x83, %eax          /* physical 0 | Present | Writable | PageSize */
        \\   mov %eax, (%edi)
        \\   /* PDPT[1] = 0x40000000 | PRESENT | WRITABLE | HUGE (1GB page) */
        \\   mov $0x40000083, %eax
        \\   mov %eax, 8(%edi)
        \\
        \\   /* Load PML4 into CR3 */
        \\   mov $_pml4, %eax
        \\   mov %eax, %cr3
        \\
        \\   /* ---------------------------------------------------------------- */
        \\   /* Enable PAE (Physical Address Extension) — required for long mode */
        \\   /* ---------------------------------------------------------------- */
        \\   mov %cr4, %eax
        \\   or  $0x20, %eax          /* CR4.PAE */
        \\   mov %eax, %cr4
        \\
        \\   /* ---------------------------------------------------------------- */
        \\   /* Enable Long Mode via EFER MSR                                   */
        \\   /* ---------------------------------------------------------------- */
        \\   mov $0xC0000080, %ecx    /* MSR: IA32_EFER */
        \\   rdmsr
        \\   or  $0x100, %eax         /* EFER.LME = 1 */
        \\   wrmsr
        \\
        \\   /* ---------------------------------------------------------------- */
        \\   /* Enable paging → activates long mode (CR0.PG, CR0.PE already set)*/
        \\   /* ---------------------------------------------------------------- */
        \\   mov %cr0, %eax
        \\   or  $0x80000000, %eax
        \\   mov %eax, %cr0
        \\
        \\   /* ---------------------------------------------------------------- */
        \\   /* Load a minimal 64-bit GDT and far-jump to 64-bit segment         */
        \\   /* ---------------------------------------------------------------- */
        \\   mov $_gdt64_ptr, %edi
        \\   lgdt (%edi)
        \\   /* Far jump: segment 0x08 (64-bit code descriptor) : _start64 */
        \\   ljmp $0x08, $_start64
        \\
        \\ .code64
        \\ _start64:
        \\   /* Reload data segment registers with 64-bit data descriptor (0x10) */
        \\   mov $0x10, %ax
        \\   mov %ax, %ds
        \\   mov %ax, %es
        \\   mov %ax, %fs
        \\   mov %ax, %gs
        \\   mov %ax, %ss
        \\
        \\   /* DEBUG: write a green 'L' to VGA at 0xB8000 */
        \\   movw $0x024C, (0xB8000)   /* 0x024C = 'L' (0x4C) | attr green(0x02) << 8 */
        \\
        \\   /* Zero BSS segment */
        \\   mov $(_bss_start), %rdi
        \\   mov $(_bss_end),   %rcx
        \\   sub %rdi, %rcx
        \\   xor %eax, %eax
        \\   rep stosb
        \\
        \\   /* Set up 64-bit kernel stack */
        \\   mov $(_boot_stack_top), %rsp
        \\
        \\   /* Call Zig kernel_main — should never return */
        \\   call kernel_main
        \\
        \\ _halt:
        \\   cli
        \\   hlt
        \\   jmp _halt
    );

    // ---------------------------------------------------------------------------
    // Bootstrap data: temporary GDT, page tables, stack
    // ---------------------------------------------------------------------------
    asm (
        \\ .section .data
        \\ .align 8
        \\
        \\ /* 64-bit GDT used during early boot only.
        \\    The real GDT is loaded by gdt_init() in Zig. */
        \\ _gdt64:
        \\   /* Null descriptor */
        \\   .quad 0
        \\   /* 64-bit code: L=1, DPL=0, Present=1 */
        \\   .quad 0x00AF9A000000FFFF
        \\   /* 64-bit data: DPL=0, Present=1 */
        \\   .quad 0x00CF92000000FFFF
        \\ _gdt64_end:
        \\
        \\ _gdt64_ptr:
        \\   .short (_gdt64_end - _gdt64 - 1)
        \\   .long  _gdt64
        \\
        \\ /* Saved Multiboot2 data */
        \\ .global mb2_info_ptr
        \\ mb2_info_ptr: .long 0
        \\ .global mb2_magic
        \\ mb2_magic:    .long 0
        \\
        \\ /* ---------------------------------------------------------------- */
        \\ /* Early boot page tables (3 pages, 4KB-aligned)                   */
        \\ /* Placed in .data (NOT .bss!) so BSS zeroing doesn't wipe them.   */
        \\ /* ---------------------------------------------------------------- */
        \\ .section .data
        \\ .align 4096
        \\ _pml4: .skip 4096
        \\ _pdpt: .skip 4096
        \\ _pd:   .skip 4096
        \\
        \\ /* 32KB kernel boot stack */
        \\ .align 16
        \\ _boot_stack_bottom:
        \\   .skip 32768
        \\ _boot_stack_top:
    );
}
