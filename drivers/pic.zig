const vga = @import("vga.zig");
const io = @import("../arch/x86_64/io.zig");

pub const IRQ0_VECTOR: u8 = 0x20;
pub const IRQ1_VECTOR: u8 = 0x21;
pub const IRQ2_VECTOR: u8 = 0x22;
pub const IRQ3_VECTOR: u8 = 0x23;
pub const IRQ4_VECTOR: u8 = 0x24;
pub const IRQ5_VECTOR: u8 = 0x25;
pub const IRQ6_VECTOR: u8 = 0x26;
pub const IRQ7_VECTOR: u8 = 0x27;
pub const IRQ8_VECTOR: u8 = 0x28;
pub const IRQ12_VECTOR: u8 = 0x2C;
pub const IRQ13_VECTOR: u8 = 0x2D;
pub const IRQ14_VECTOR: u8 = 0x2E;
pub const IRQ15_VECTOR: u8 = 0x2F;

// Remove local outb, inb, io_wait

pub fn pic_init() void {
    const mask1 = io.inb(0x21);
    const mask2 = io.inb(0xA1);

    io.outb(0x20, 0x11);
    io.io_wait();
    io.outb(0xA0, 0x11);
    io.io_wait();
    io.outb(0x21, 0x20);
    io.io_wait();
    io.outb(0xA1, 0x28);
    io.io_wait();
    io.outb(0x21, 0x04);
    io.io_wait();
    io.outb(0xA1, 0x02);
    io.io_wait();
    io.outb(0x21, 0x01);
    io.io_wait();
    io.outb(0xA1, 0x01);
    io.io_wait();
    io.outb(0x21, mask1);
    io.outb(0xA1, mask2);

    vga.set_color(.Green, .Black);
    vga.print("[PIC] 8259A initialized -- IRQs remapped.\n");
    vga.set_color(.White, .Black);
}

pub fn set_master_mask(mask: u8) void {
    io.outb(0x21, mask);
}
pub fn set_slave_mask(mask: u8) void {
    io.outb(0xA1, mask);
}
pub fn eoi_master() void {
    io.outb(0x20, 0x20);
}
pub fn eoi_slave() void {
    io.outb(0xA0, 0x20);
    io.outb(0x20, 0x20);
}
