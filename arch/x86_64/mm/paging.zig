// arch/x86_64/mm/paging.zig
// Written in Zig
// x86_64 Page Map Level 4 table setup.

const std = @import("std");

pub const PAGE_SIZE: usize = 4096;

pub const PageTableEntry = struct {
    physical_address: usize,
    present: bool,
    writeable: bool,
    user_accessible: bool,
    executable: bool,
};

pub const PageTable = struct {
    entries: [512]PageTableEntry,
};

pub const PagingContext = struct {
    pml4: PageTable,
    
    pub fn init() PagingContext {
        return PagingContext {
            .pml4 = PageTable {
                .entries = [_]PageTableEntry{ PageTableEntry{
                    .physical_address = 0,
                    .present = false,
                    .writeable = false,
                    .user_accessible = false,
                    .executable = false,
                } } ** 512,
            },
        };
    }

    pub fn map(self: *PagingContext, virtual_addr: usize, physical_addr: usize, flags: u32) !void {
        const pml4_index = (virtual_addr >> 39) & 0x1FF;
        const pdpt_index = (virtual_addr >> 30) & 0x1FF;
        const pd_index = (virtual_addr >> 21) & 0x1FF;
        const pt_index = (virtual_addr >> 12) & 0x1FF;

        self.pml4.entries[pml4_index].present = true;
        self.pml4.entries[pml4_index].writeable = (flags & 1) != 0;
        self.pml4.entries[pml4_index].user_accessible = (flags & 2) != 0;
        self.pml4.entries[pml4_index].physical_address = physical_addr;

        std.debug.print("[arch/x86_64/mm/paging] Map virtual 0x{x} -> physical 0x{x} [PML4[{}] -> PDPT[{}] -> PD[{}] -> PT[{}]]\n", .{
            virtual_addr, physical_addr, pml4_index, pdpt_index, pd_index, pt_index
        });
        invlpg(virtual_addr);
    }

    pub fn unmap(self: *PagingContext, virtual_addr: usize) void {
        const pml4_index = (virtual_addr >> 39) & 0x1FF;
        self.pml4.entries[pml4_index].present = false;
        std.debug.print("[arch/x86_64/mm/paging] Unmap virtual 0x{x}\n", .{virtual_addr});
        invlpg(virtual_addr);
    }
};

inline fn invlpg(virtual_addr: usize) void {
    asm volatile ("invlpg (%rdi)"
        :
        : [addr] "{rdi}" (virtual_addr)
        : "memory"
    );
}

var kernel_paging_context: PagingContext = undefined;
var paging_initialized: bool = false;

pub fn paging_global_init() void {
    kernel_paging_context = PagingContext.init();
    paging_initialized = true;
}

pub export fn page_table_map(virtual_addr: usize, physical_addr: usize, flags: u32) i32 {
    if (!paging_initialized) paging_global_init();
    kernel_paging_context.map(virtual_addr, physical_addr, flags) catch return -1;
    return 0;
}

pub export fn page_table_unmap(virtual_addr: usize) void {
    if (!paging_initialized) paging_global_init();
    kernel_paging_context.unmap(virtual_addr);
}
