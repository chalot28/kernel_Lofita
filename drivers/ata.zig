const io = @import("../arch/x86_64/io.zig");
const vga = @import("vga.zig");

// Primary ATA Bus Ports
const DATA_PORT = 0x1F0;
const ERROR_PORT = 0x1F1;
const SECT_COUNT_PORT = 0x1F2;
const LBA_LO_PORT = 0x1F3;
const LBA_MID_PORT = 0x1F4;
const LBA_HI_PORT = 0x1F5;
const DRIVE_PORT = 0x1F6;
const COMMAND_PORT = 0x1F7;
const STATUS_PORT = 0x1F7;

const CMD_IDENTIFY = 0xEC;
const CMD_READ_PIO = 0x20;
const CMD_WRITE_PIO = 0x30;

const STATUS_BSY = 0x80;
const STATUS_DRQ = 0x08;
const STATUS_ERR = 0x01;

pub fn ata_wait_bsy() void {
    while ((io.inb(STATUS_PORT) & STATUS_BSY) != 0) {}
}

pub fn ata_wait_drq() void {
    while ((io.inb(STATUS_PORT) & (STATUS_BSY | STATUS_DRQ)) != STATUS_DRQ) {
        if ((io.inb(STATUS_PORT) & STATUS_ERR) != 0) {
            vga.print("[ATA] Error during DRQ wait!\n");
            break;
        }
    }
}

pub fn ata_init() void {
    // Select Drive 0 (Master)
    io.outb(DRIVE_PORT, 0xA0);
    
    // Set sectorcount, lba lo, mid, hi to 0
    io.outb(SECT_COUNT_PORT, 0);
    io.outb(LBA_LO_PORT, 0);
    io.outb(LBA_MID_PORT, 0);
    io.outb(LBA_HI_PORT, 0);
    
    // Send IDENTIFY
    io.outb(COMMAND_PORT, CMD_IDENTIFY);
    
    const status = io.inb(STATUS_PORT);
    if (status == 0) {
        vga.print("[ATA] Primary Master Drive does NOT exist.\n");
        return;
    }
    
    ata_wait_bsy();
    
    // Check for non-ATA devices (ATAPI/CD-ROM)
    const mid = io.inb(LBA_MID_PORT);
    const hi = io.inb(LBA_HI_PORT);
    if (mid != 0 or hi != 0) {
        vga.print("[ATA] Primary Master Drive is not ATA (might be ATAPI).\n");
        return;
    }
    
    ata_wait_drq();
    
    // Read IDENTIFY data
    var buf: [256]u16 = undefined;
    var i: usize = 0;
    while (i < 256) : (i += 1) {
        buf[i] = io.inw(DATA_PORT);
    }
    
    vga.set_color(.LightBlue, .Black);
    vga.print("[ATA] Primary Master Drive initialized.\n");
    vga.set_color(.White, .Black);
}

// C-ABI exported function for Rust VFS to call
pub export fn ata_read_sectors(lba: u32, sector_count: u8, dest: [*]u8) void {
    // Select Drive 0, LBA mode
    io.outb(DRIVE_PORT, 0xE0 | (@as(u8, @truncate((lba >> 24) & 0x0F))));
    io.outb(SECT_COUNT_PORT, sector_count);
    io.outb(LBA_LO_PORT, @as(u8, @truncate(lba & 0xFF)));
    io.outb(LBA_MID_PORT, @as(u8, @truncate((lba >> 8) & 0xFF)));
    io.outb(LBA_HI_PORT, @as(u8, @truncate((lba >> 16) & 0xFF)));
    
    // Send READ command
    io.outb(COMMAND_PORT, CMD_READ_PIO);
    
    var ptr: [*]u16 = @ptrCast(@alignCast(dest));
    var sectors_read: u8 = 0;
    
    while (sectors_read < sector_count) : (sectors_read += 1) {
        ata_wait_bsy();
        ata_wait_drq();
        
        var i: usize = 0;
        while (i < 256) : (i += 1) {
            ptr[i] = io.inw(DATA_PORT);
        }
        ptr += 256;
    }
}

pub export fn ata_write_sectors(lba: u32, sector_count: u8, src: [*]const u8) void {
    io.outb(DRIVE_PORT, 0xE0 | (@as(u8, @truncate((lba >> 24) & 0x0F))));
    io.outb(SECT_COUNT_PORT, sector_count);
    io.outb(LBA_LO_PORT, @as(u8, @truncate(lba & 0xFF)));
    io.outb(LBA_MID_PORT, @as(u8, @truncate((lba >> 8) & 0xFF)));
    io.outb(LBA_HI_PORT, @as(u8, @truncate((lba >> 16) & 0xFF)));
    
    io.outb(COMMAND_PORT, CMD_WRITE_PIO);
    
    var ptr: [*]const u16 = @ptrCast(@alignCast(src));
    var sectors_written: u8 = 0;
    
    while (sectors_written < sector_count) : (sectors_written += 1) {
        ata_wait_bsy();
        ata_wait_drq();
        
        var i: usize = 0;
        while (i < 256) : (i += 1) {
            io.outw(DATA_PORT, ptr[i]);
        }
        ptr += 256;
    }
    
    // Cache flush? Not strictly required for basic PIO, but good practice.
}
