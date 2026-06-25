// compat/linux/src/elf_loader.rs
// Written in Rust (no_std)
// ELF64 binary loader for the Lofita Linux compatibility layer.
//
// Reads an ELF64 file from the VFS, parses its program headers,
// allocates physical pages via the kernel allocator, and maps each
// PT_LOAD segment into the process's virtual address space.
//
// References:
//   System V ABI — ELF-64 Object File Format, Version 1.5 Draft 2
//   https://uclibc.org/docs/elf-64-gen.pdf


use crate::kprint;

// ─── ELF64 Header ────────────────────────────────────────────────────────────

const ELF_MAGIC:     [u8; 4] = [0x7F, b'E', b'L', b'F'];
const ELFCLASS64:    u8      = 2;
const ELFDATA2LSB:   u8      = 1; // Little-endian
const ET_EXEC:       u16     = 2; // Executable file
const ET_DYN:        u16     = 3; // Shared object (PIE executables use this)
const EM_X86_64:     u16     = 62;
const PT_LOAD:       u32     = 1; // Loadable segment

/// ELF64 file header (64 bytes)
#[repr(C, packed)]
struct Elf64Header {
    e_ident:     [u8; 16], // Magic + class + data + version + OS/ABI + padding
    e_type:      u16,      // Object file type
    e_machine:   u16,      // Target ISA
    e_version:   u32,      // ELF version (always 1)
    e_entry:     u64,      // Virtual address of entry point
    e_phoff:     u64,      // File offset of program header table
    e_shoff:     u64,      // File offset of section header table (unused here)
    e_flags:     u32,      // Architecture-specific flags
    e_ehsize:    u16,      // Size of this header (64 bytes)
    e_phentsize: u16,      // Size of one program header entry (56 bytes)
    e_phnum:     u16,      // Number of program header entries
    e_shentsize: u16,      // Size of one section header entry
    e_shnum:     u16,      // Number of section header entries
    e_shstrndx:  u16,      // Section name string table index
}

/// ELF64 program header (56 bytes per entry)
#[repr(C, packed)]
struct Elf64ProgramHeader {
    p_type:   u32, // Segment type (PT_LOAD = 1)
    p_flags:  u32, // Segment permissions (PF_X=1, PF_W=2, PF_R=4)
    p_offset: u64, // Offset of segment data in the file
    p_vaddr:  u64, // Virtual address to load at
    p_paddr:  u64, // Physical address (usually = p_vaddr)
    p_filesz: u64, // Size of segment in the file
    p_memsz:  u64, // Size of segment in memory (>= p_filesz, extra bytes zeroed)
    p_align:  u64, // Required alignment
}

// Segment permission flags
const PF_EXEC:  u32 = 1;
const PF_WRITE: u32 = 2;
const PF_READ:  u32 = 4;

// ─── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ElfError {
    TooShort,
    BadMagic,
    NotElf64,
    NotLittleEndian,
    NotExecutable,   // Not ET_EXEC or ET_DYN
    WrongArch,       // Not x86_64
    BadProgramHeader,
    AllocFailed,
    MapFailed,
}

// ─── Loaded Process Descriptor ───────────────────────────────────────────────

pub struct LoadedProcess {
    /// Entry point virtual address
    pub entry_point:  u64,
    /// Base virtual address (for PIE — dynamic executables)
    pub load_base:    u64,
    /// Initial stack pointer (to be set up by scheduler)
    pub initial_rsp:  u64,
}

// ─── Main Loader ─────────────────────────────────────────────────────────────

/// Load an ELF64 binary from a byte slice into the current address space.
///
/// Allocates physical pages and maps them via the kernel's page table manager.
/// Returns a `LoadedProcess` describing the entry point and initial stack.
pub fn load_elf(elf_data: &[u8]) -> Result<LoadedProcess, ElfError> {
    // ── 1. Validate ELF header ──────────────────────────────────────────────

    if elf_data.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::TooShort);
    }

    // Parse header by reading bytes directly (avoids alignment issues on packed structs)
    let magic = &elf_data[0..4];
    if magic != ELF_MAGIC {
        return Err(ElfError::BadMagic);
    }
    if elf_data[4] != ELFCLASS64 {
        return Err(ElfError::NotElf64);
    }
    if elf_data[5] != ELFDATA2LSB {
        return Err(ElfError::NotLittleEndian);
    }

    let e_type    = read_u16_le(elf_data, 16);
    let e_machine = read_u16_le(elf_data, 18);
    let e_entry   = read_u64_le(elf_data, 24);
    let e_phoff   = read_u64_le(elf_data, 32) as usize;
    let e_phentsize = read_u16_le(elf_data, 54) as usize;
    let e_phnum   = read_u16_le(elf_data, 56) as usize;

    if e_type != ET_EXEC && e_type != ET_DYN {
        return Err(ElfError::NotExecutable);
    }
    if e_machine != EM_X86_64 {
        return Err(ElfError::WrongArch);
    }

    kprint!("[ELF] Type={} Machine=x86_64 Entry=0x{:x} PHdr@0x{:x} ({} entries)\n",
        e_type, e_entry, e_phoff, e_phnum);

    // ── 2. Parse program headers and load PT_LOAD segments ──────────────────

    let mut load_base:    u64 = 0;
    let mut min_vaddr:    u64 = u64::MAX;
    let mut max_vaddr:    u64 = 0;

    // First pass: find virtual address range (needed for PIE base calculation)
    for i in 0..e_phnum {
        let ph_off = e_phoff + i * e_phentsize;
        if ph_off + e_phentsize > elf_data.len() {
            return Err(ElfError::BadProgramHeader);
        }
        let p_type  = read_u32_le(elf_data, ph_off);
        let p_vaddr = read_u64_le(elf_data, ph_off + 16);
        let p_memsz = read_u64_le(elf_data, ph_off + 40);
        if p_type == PT_LOAD {
            if p_vaddr < min_vaddr { min_vaddr = p_vaddr; }
            if p_vaddr + p_memsz > max_vaddr { max_vaddr = p_vaddr + p_memsz; }
        }
    }

    // PIE: if min_vaddr == 0 the binary is position-independent → apply a base
    if e_type == ET_DYN && min_vaddr == 0 {
        load_base = 0x4000_0000; // Load PIE at 1GB virtual
        kprint!("[ELF] PIE binary — load base 0x{:x}\n", load_base);
    }

    // Second pass: allocate and map each PT_LOAD segment
    for i in 0..e_phnum {
        let ph_off  = e_phoff + i * e_phentsize;
        let p_type   = read_u32_le(elf_data, ph_off);
        if p_type != PT_LOAD { continue; }

        let p_flags  = read_u32_le(elf_data, ph_off + 4);
        let p_offset = read_u64_le(elf_data, ph_off + 8) as usize;
        let p_vaddr  = read_u64_le(elf_data, ph_off + 16);
        let p_filesz = read_u64_le(elf_data, ph_off + 32) as usize;
        let p_memsz  = read_u64_le(elf_data, ph_off + 40) as usize;

        let is_exec  = (p_flags & PF_EXEC)  != 0;
        let is_write = (p_flags & PF_WRITE) != 0;

        let virt_base = load_base + p_vaddr;
        let pages     = (p_memsz + 4095) / 4096;

        kprint!("[ELF] PT_LOAD seg {}: vaddr=0x{:x} filesz={} memsz={} [{}{}{}]\n",
            i, virt_base, p_filesz, p_memsz,
            if is_exec  {'X'} else {'-'},
            if is_write {'W'} else {'-'},
            if p_flags & PF_READ != 0 {'R'} else {'-'},
        );

        // Allocate physical pages
        let phys = unsafe { crate::vasm::phys_alloc(pages) };
        if phys.is_null() {
            kprint!("[ELF] Out of physical memory for segment {}!\n", i);
            return Err(ElfError::AllocFailed);
        }

        // Copy file data into physical pages
        let phys_slice = unsafe {
            core::slice::from_raw_parts_mut(phys, p_memsz)
        };
        let file_end = core::cmp::min(p_offset + p_filesz, elf_data.len());
        let copy_len = file_end - p_offset;
        phys_slice[0..copy_len].copy_from_slice(&elf_data[p_offset..file_end]);
        // Zero-fill BSS portion (p_memsz > p_filesz)
        for b in phys_slice[copy_len..].iter_mut() {
            *b = 0;
        }

        // Map pages into virtual address space
        let mut pte_flags: u32 = 1 << 1; // User accessible
        if is_write { pte_flags |= 1 << 0; } // Writable
        if is_exec  { pte_flags |= 1 << 2; } // Executable

        for page_i in 0..pages {
            let vpage = virt_base + (page_i * 4096) as u64;
            let ppage = phys as usize + page_i * 4096;
            unsafe { crate::vasm::page_table_map(vpage as usize, ppage, pte_flags) };
        }
    }

    // ── 3. Set up initial user stack ─────────────────────────────────────────
    // Allocate 2 pages (8KB) for the initial user stack at the top of
    // the 32-bit-compatible virtual address range.
    let stack_pages  = 2usize;
    let stack_phys   = unsafe { crate::vasm::phys_alloc(stack_pages) };
    let stack_virt   = 0x7FFF_F000u64 - (stack_pages * 4096) as u64;

    for p in 0..stack_pages {
        let vp = stack_virt + (p * 4096) as u64;
        let pp = stack_phys as usize + p * 4096;
        unsafe { crate::vasm::page_table_map(vp as usize, pp, 0x7); } // RW, User (Flags: 7 = Present | R/W | User)
    }

    let initial_rsp = stack_virt + (stack_pages * 4096) as u64 - 8; // 8-byte aligned

    kprint!("[ELF] Load complete. Entry=0x{:x} RSP=0x{:x}\n",
        load_base + e_entry, initial_rsp);

    Ok(LoadedProcess {
        entry_point: load_base + e_entry,
        load_base,
        initial_rsp,
    })
}

// ─── Little-endian byte readers ──────────────────────────────────────────────

#[inline(always)]
fn read_u16_le(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

#[inline(always)]
fn read_u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
}

#[inline(always)]
fn read_u64_le(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        data[off], data[off+1], data[off+2], data[off+3],
        data[off+4], data[off+5], data[off+6], data[off+7],
    ])
}
