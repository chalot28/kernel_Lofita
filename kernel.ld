/* kernel.ld — Lofita OS Linker Script
 * Places the kernel binary at 1MB physical address (0x100000).
 * GRUB Multiboot2 requires the Multiboot header to appear within
 * the first 32768 bytes of the kernel image.
 *
 * Memory layout:
 *   0x000000 - 0x0FFFFF : BIOS / Legacy area (off-limits)
 *   0x100000 +          : Lofita kernel (loaded here by GRUB)
 */

ENTRY(_start)

SECTIONS {
    /* ------------------------------------------------------------------ */
    /* Kernel base: 1MB physical = 1MB virtual (no higher-half mapping yet) */
    /* ------------------------------------------------------------------ */
    . = 0x100000;

    /* Multiboot2 header MUST be first — GRUB scans here */
    .multiboot ALIGN(8) : {
        KEEP(*(.multiboot))
    }

    /* Executable code */
    .text ALIGN(4096) : {
        *(.text .text.*)
    }

    /* Read-only data (string literals, vtables, etc.) */
    .rodata ALIGN(4096) : {
        *(.rodata .rodata.*)
    }

    /* Initialized data */
    .data ALIGN(4096) : {
        *(.data .data.*)
    }

    /* Uninitialized data — zeroed by boot stub */
    .bss ALIGN(4096) : {
        _bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        _bss_end = .;
    }

    /* Export kernel end address for heap setup */
    _kernel_end = .;

    /* ------------------------------------------------------------------ */
    /* Discard unnecessary sections to keep the image lean                  */
    /* ------------------------------------------------------------------ */
    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
        *(.debug*)
    }
}
