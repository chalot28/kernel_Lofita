# boot/multiboot.s -- Lofita OS Multiboot2 Boot Stub
# Multiboot header placed at the very beginning of .text so GRUB
# finds it immediately.

.section .text.startup, "ax"
.global _start

# Multiboot2 Header (must be within first 32768 bytes)
.align 8
multiboot2_header_start:
.long 0xE85250D6          # magic
.long 0                   # architecture: i386 protected mode
.long multiboot2_header_end - multiboot2_header_start
.long -(0xE85250D6 + 0 + (multiboot2_header_end - multiboot2_header_start))

# Information request tag (type=1): ask GRUB for memory map
.align 8
.short 1                  # tag type: information request
.short 0                  # flags
.long  12                 # size
.long  6                  # MBI type 6 = memory map

# End tag
.align 8
.short 0                  # type = 0 (end)
.short 0                  # flags = 0
.long  8                  # size = 8
multiboot2_header_end:

# ---------------------------------------------------------------------------
# 32-bit bootstrap: _start (entered from GRUB in 32-bit protected mode)
# ---------------------------------------------------------------------------
.code32
_start:
  cli
  cld

  # Save multiboot2 info ptr (EBX) and magic (EAX)
  mov $mb2_info_ptr, %edi
  mov %ebx, (%edi)
  mov $mb2_magic, %edi
  mov %eax, (%edi)

  # Set up temporary 32-bit stack
  mov $_boot_stack_top, %esp

  # Enable SSE
  mov %cr0, %eax
  and $0xFFFFFFFB, %eax
  or  $0x00000002, %eax
  mov %eax, %cr0
  mov %cr4, %eax
  or  $0x00000600, %eax
  mov %eax, %cr4

  # Build identity-mapped page tables for the first 2 GB
  mov $_pml4, %edi
  xor %eax, %eax
  mov $3072, %ecx
  rep stosl

  # PML4[0] = &PDPT | PRESENT | WRITABLE
  mov $_pml4, %edi
  mov $_pdpt, %eax
  or  $0x3, %eax
  mov %eax, (%edi)

  # Map first 2GB via 1GB huge pages in PDPT
  mov $_pdpt, %edi
  mov $0x83, %eax
  mov %eax, (%edi)
  mov $0x40000083, %eax
  mov %eax, 8(%edi)

  # Load PML4 into CR3
  mov $_pml4, %eax
  mov %eax, %cr3

  # Enable PAE (required for long mode)
  mov %cr4, %eax
  or  $0x20, %eax
  mov %eax, %cr4

  # Enable Long Mode via EFER MSR
  mov $0xC0000080, %ecx
  rdmsr
  or  $0x100, %eax
  wrmsr

  # Enable paging (activates long mode)
  mov %cr0, %eax
  or  $0x80000000, %eax
  mov %eax, %cr0

  # Load minimal 64-bit GDT and far-jump
  mov $_gdt64_ptr, %edi
  lgdt (%edi)
  ljmp $0x08, $_start64

.code64
_start64:
  # Reload data segment registers
  mov $0x10, %ax
  mov %ax, %ds
  mov %ax, %es
  mov %ax, %fs
  mov %ax, %gs
  mov %ax, %ss

  # Set up 64-bit kernel stack
  mov $(_boot_stack_top), %rsp

  # Call Zig kernel_main -- should never return
  call kernel_main

_halt:
  cli
  hlt
  jmp _halt

# ---------------------------------------------------------------------------
# Bootstrap data: temporary GDT, page tables, stack
# ---------------------------------------------------------------------------
.section .data
.align 8

_gdt64:
  .quad 0
  .quad 0x00AF9A000000FFFF
  .quad 0x00CF92000000FFFF
_gdt64_end:

_gdt64_ptr:
  .short (_gdt64_end - _gdt64 - 1)
  .long  _gdt64

.global mb2_info_ptr
mb2_info_ptr: .long 0
.global mb2_magic
mb2_magic:    .long 0

.align 4096
_pml4: .skip 4096
_pdpt: .skip 4096
_pd:   .skip 4096

.align 16
_boot_stack_bottom:
  .skip 32768
_boot_stack_top:
