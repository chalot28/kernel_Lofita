// drivers/serial.zig — Lofita OS 16550 UART Serial Driver
// For use with QEMU -nographic / -serial stdio

const vga = @import("vga.zig");

const io = @import("../arch/x86_64/io.zig");

const COM1 = 0x3F8;

pub fn serial_init() void {
    io.outb(COM1 + 1, 0x00); // Disable all interrupts
    io.outb(COM1 + 3, 0x80); // Enable DLAB (set baud rate divisor)
    io.outb(COM1 + 0, 0x03); // Set divisor to 3 (38400 baud)
    io.outb(COM1 + 1, 0x00); // High byte of divisor
    io.outb(COM1 + 3, 0x03); // 8 bits, no parity, 1 stop bit
    io.outb(COM1 + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
    io.outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set

    vga.print("[SERIAL] COM1 initialized at 38400 baud.\n");
}

pub fn serial_print(msg: []const u8) void {
    for (msg) |c| {
        // Wait for transmitter holding register to be empty
        while (io.inb(COM1 + 5) & 0x20 == 0) {}
        io.outb(COM1 + 0, c);
    }
}

pub fn serial_read() ?u8 {
    // Check if data is ready (bit 0 of Line Status Register)
    if (io.inb(COM1 + 5) & 0x01 != 0) {
        return io.inb(COM1 + 0);
    }
    return null;
}
