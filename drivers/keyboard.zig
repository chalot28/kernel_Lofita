const vga = @import("vga.zig");
const pic = @import("pic.zig");

const RING_BUF_SIZE = 256;
var ring_buf: [RING_BUF_SIZE]u8 = undefined;
var ring_head: usize = 0;
var ring_tail: usize = 0;

fn ring_put(c: u8) void {
    const next = (ring_head + 1) % RING_BUF_SIZE;
    if (next != ring_tail) {
        ring_buf[ring_head] = c;
        ring_head = next;
    }
}

pub fn read_key() ?u8 {
    if (ring_head == ring_tail) return null;
    const c = ring_buf[ring_tail];
    ring_tail = (ring_tail + 1) % RING_BUF_SIZE;
    return c;
}

fn outb(port: u16, val: u8) void {
    asm volatile ("outb %[val], %[port]"
        :
        : [val] "{al}" (val),
          [port] "{dx}" (port),
    );
}

fn inb(port: u16) u8 {
    return asm volatile ("inb %[port], %[result]"
        : [result] "={al}" (-> u8),
        : [port] "{dx}" (port),
    );
}

fn wait_write() void {
    while (inb(0x64) & 0x02 != 0) {}
}
fn wait_read() void {
    while (inb(0x64) & 0x01 == 0) {}
}

const SCANCODE_NORMAL: [128]u8 = blk: {
    var t: [128]u8 = @splat(0);
    t[0x01] = 0x1B;
    t[0x02] = '1';
    t[0x03] = '2';
    t[0x04] = '3';
    t[0x05] = '4';
    t[0x06] = '5';
    t[0x07] = '6';
    t[0x08] = '7';
    t[0x09] = '8';
    t[0x0A] = '9';
    t[0x0B] = '0';
    t[0x0C] = '-';
    t[0x0D] = '=';
    t[0x0E] = 0x08;
    t[0x0F] = '\t';
    t[0x10] = 'q';
    t[0x11] = 'w';
    t[0x12] = 'e';
    t[0x13] = 'r';
    t[0x14] = 't';
    t[0x15] = 'y';
    t[0x16] = 'u';
    t[0x17] = 'i';
    t[0x18] = 'o';
    t[0x19] = 'p';
    t[0x1A] = '[';
    t[0x1B] = ']';
    t[0x1C] = '\n';
    t[0x1E] = 'a';
    t[0x1F] = 's';
    t[0x20] = 'd';
    t[0x21] = 'f';
    t[0x22] = 'g';
    t[0x23] = 'h';
    t[0x24] = 'j';
    t[0x25] = 'k';
    t[0x26] = 'l';
    t[0x27] = ';';
    t[0x28] = '\'';
    t[0x29] = '`';
    t[0x2B] = '\\';
    t[0x2C] = 'z';
    t[0x2D] = 'x';
    t[0x2E] = 'c';
    t[0x2F] = 'v';
    t[0x30] = 'b';
    t[0x31] = 'n';
    t[0x32] = 'm';
    t[0x33] = ',';
    t[0x34] = '.';
    t[0x35] = '/';
    t[0x39] = ' ';
    break :blk t;
};

const SCANCODE_SHIFT: [128]u8 = blk: {
    var t: [128]u8 = @splat(0);
    t[0x02] = '!';
    t[0x03] = '@';
    t[0x04] = '#';
    t[0x05] = '$';
    t[0x06] = '%';
    t[0x07] = '^';
    t[0x08] = '&';
    t[0x09] = '*';
    t[0x0A] = '(';
    t[0x0B] = ')';
    t[0x0C] = '_';
    t[0x0D] = '+';
    t[0x0E] = 0x08;
    t[0x0F] = '\t';
    t[0x10] = 'Q';
    t[0x11] = 'W';
    t[0x12] = 'E';
    t[0x13] = 'R';
    t[0x14] = 'T';
    t[0x15] = 'Y';
    t[0x16] = 'U';
    t[0x17] = 'I';
    t[0x18] = 'O';
    t[0x19] = 'P';
    t[0x1A] = '{';
    t[0x1B] = '}';
    t[0x1C] = '\n';
    t[0x1E] = 'A';
    t[0x1F] = 'S';
    t[0x20] = 'D';
    t[0x21] = 'F';
    t[0x22] = 'G';
    t[0x23] = 'H';
    t[0x24] = 'J';
    t[0x25] = 'K';
    t[0x26] = 'L';
    t[0x27] = ':';
    t[0x28] = '"';
    t[0x29] = '~';
    t[0x2B] = '|';
    t[0x2C] = 'Z';
    t[0x2D] = 'X';
    t[0x2E] = 'C';
    t[0x2F] = 'V';
    t[0x30] = 'B';
    t[0x31] = 'N';
    t[0x32] = 'M';
    t[0x33] = '<';
    t[0x34] = '>';
    t[0x35] = '?';
    t[0x39] = ' ';
    break :blk t;
};

var shift_pressed: bool = false;
var ctrl_pressed: bool = false;
var alt_pressed: bool = false;
var caps_lock: bool = false;

const SCANCODE_LSHIFT = 0x2A;
const SCANCODE_RSHIFT = 0x36;
const SCANCODE_LCTRL = 0x1D;
const SCANCODE_LALT = 0x38;
const SCANCODE_CAPSLOCK = 0x3A;

pub fn handle_keyboard_interrupt() callconv(.c) void {
    const scancode = inb(0x60);
    if (scancode == 0xE0) {
        pic.eoi_master();
        return;
    }

    const is_release = (scancode & 0x80) != 0;
    const code = scancode & 0x7F;

    if (code == SCANCODE_LSHIFT or code == SCANCODE_RSHIFT) {
        shift_pressed = !is_release;
        pic.eoi_master();
        return;
    }
    if (code == SCANCODE_LCTRL) {
        ctrl_pressed = !is_release;
        pic.eoi_master();
        return;
    }
    if (code == SCANCODE_LALT) {
        alt_pressed = !is_release;
        pic.eoi_master();
        return;
    }
    if (code == SCANCODE_CAPSLOCK and !is_release) {
        caps_lock = !caps_lock;
        wait_write();
        outb(0x64, 0xED);
        wait_write();
        outb(0x60, if (caps_lock) @as(u8, 4) else 0);
        pic.eoi_master();
        return;
    }
    if (is_release) {
        pic.eoi_master();
        return;
    }

    var ch: u8 = if (shift_pressed) SCANCODE_SHIFT[code] else SCANCODE_NORMAL[code];
    if (caps_lock and ch >= 'a' and ch <= 'z') ch -= 32;
    if (caps_lock and ch >= 'A' and ch <= 'Z') ch += 32;
    if (ch != 0) ring_put(ch);
    pic.eoi_master();
}

pub fn keyboard_init() void {
    shift_pressed = false;
    ctrl_pressed = false;
    alt_pressed = false;
    caps_lock = false;
    ring_head = 0;
    ring_tail = 0;

    wait_write();
    outb(0x64, 0xAE);
    wait_write();
    outb(0x64, 0x20);
    wait_read();
    const cmd_byte = inb(0x60);
    wait_write();
    outb(0x64, 0x60);
    wait_write();
    outb(0x60, cmd_byte | 0x01);

    pic.set_master_mask(0xFD);
    pic.set_slave_mask(0xFF);

    vga.set_color(.Green, .Black);
    vga.print("[KB] PS/2 keyboard initialized -- IRQ1 active.\n");
    vga.set_color(.White, .Black);
}
