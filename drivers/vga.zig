// drivers/vga.zig — Lofita OS VGA Text Mode Driver
// Written in Zig (freestanding, no std)
//
// Drives the VGA text buffer at physical address 0xB8000.
// In 80x25 text mode each cell is 2 bytes:
//   byte 0: ASCII character code
//   byte 1: attribute (high nibble = background, low nibble = foreground)
//
// Color codes follow VGA palette:
//   0=Black 1=Blue 2=Green 3=Cyan 4=Red 5=Magenta 6=Brown 7=LightGrey
//   8=DarkGrey 9=LightBlue A=LightGreen B=LightCyan C=LightRed
//   D=LightMagenta E=Yellow F=White

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const VGA_WIDTH: usize = 80;
pub const VGA_HEIGHT: usize = 25;
pub const VGA_BASE: usize = 0xB8000;

pub const Color = enum(u8) {
    Black       = 0,
    Blue        = 1,
    Green       = 2,
    Cyan        = 3,
    Red         = 4,
    Magenta     = 5,
    Brown       = 6,
    LightGrey   = 7,
    DarkGrey    = 8,
    LightBlue   = 9,
    LightGreen  = 10,
    LightCyan   = 11,
    LightRed    = 12,
    LightMagenta= 13,
    Yellow      = 14,
    White       = 15,
};

fn make_attr(fg: Color, bg: Color) u8 {
    return @intFromEnum(fg) | (@intFromEnum(bg) << 4);
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

var cursor_col: usize = 0;
var cursor_row: usize = 0;
var default_attr: u8 = make_attr(.White, .Black);

// ---------------------------------------------------------------------------
// Low-level cell access
// ---------------------------------------------------------------------------

inline fn vga_ptr() [*]volatile u16 {
    return @ptrFromInt(VGA_BASE);
}

/// Write a single character cell at (col, row) with the given attribute byte.
pub fn put_char_at(col: usize, row: usize, char: u8, attr: u8) void {
    const index = row * VGA_WIDTH + col;
    const cell: u16 = @as(u16, char) | (@as(u16, attr) << 8);
    vga_ptr()[index] = cell;
}

// ---------------------------------------------------------------------------
// Screen management
// ---------------------------------------------------------------------------

/// Clear the entire screen with spaces using the default attribute.
pub fn clear() void {
    var r: usize = 0;
    while (r < VGA_HEIGHT) : (r += 1) {
        var c: usize = 0;
        while (c < VGA_WIDTH) : (c += 1) {
            put_char_at(c, r, ' ', default_attr);
        }
    }
    cursor_col = 0;
    cursor_row = 0;
}

/// Scroll the screen up by one line, clearing the bottom row.
fn scroll() void {
    // Move rows 1..HEIGHT-1 up to rows 0..HEIGHT-2
    var r: usize = 1;
    while (r < VGA_HEIGHT) : (r += 1) {
        var c: usize = 0;
        while (c < VGA_WIDTH) : (c += 1) {
            const src = (r) * VGA_WIDTH + c;
            const dst = (r - 1) * VGA_WIDTH + c;
            vga_ptr()[dst] = vga_ptr()[src];
        }
    }
    // Clear the last row
    var c: usize = 0;
    while (c < VGA_WIDTH) : (c += 1) {
        put_char_at(c, VGA_HEIGHT - 1, ' ', default_attr);
    }
    if (cursor_row > 0) cursor_row -= 1;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialize VGA: clear screen and position cursor at (0,0).
pub fn init() void {
    default_attr = make_attr(.White, .Black);
    clear();
}

/// Write a single character to the current cursor position, handling
/// newlines, carriage returns, and automatic scrolling.
pub fn put_char(char: u8) void {
    switch (char) {
        '\n' => {
            cursor_col = 0;
            cursor_row += 1;
        },
        '\r' => {
            cursor_col = 0;
        },
        '\t' => {
            // Advance to next 8-column tab stop
            cursor_col = (cursor_col + 8) & ~@as(usize, 7);
        },
        0x08 => { // Backspace
            if (cursor_col > 0) {
                cursor_col -= 1;
                put_char_at(cursor_col, cursor_row, ' ', default_attr);
            }
        },
        else => {
            put_char_at(cursor_col, cursor_row, char, default_attr);
            cursor_col += 1;
        },
    }

    // Wrap columns
    if (cursor_col >= VGA_WIDTH) {
        cursor_col = 0;
        cursor_row += 1;
    }

    // Scroll if past last row
    if (cursor_row >= VGA_HEIGHT) {
        scroll();
    }
}

/// Print a slice of bytes to the VGA buffer.
pub fn print(msg: []const u8) void {
    for (msg) |c| {
        put_char(c);
    }
}

/// Print a null-terminated C string to the VGA buffer.
pub fn print_cstr(msg: [*:0]const u8) void {
    var i: usize = 0;
    while (msg[i] != 0) : (i += 1) {
        put_char(msg[i]);
    }
}

/// Print a u64 as a hexadecimal string prefixed with "0x".
pub fn print_hex(value: u64) void {
    const digits = "0123456789ABCDEF";
    print("0x");
    var shift: i8 = 60;
    var started = false;
    while (shift >= 0) : (shift -= 4) {
        const nibble: u8 = @intCast((value >> @intCast(shift)) & 0xF);
        if (nibble != 0 or started or shift == 0) {
            put_char(digits[nibble]);
            started = true;
        }
    }
}

/// Print a u64 as a decimal string.
pub fn print_dec(value: u64) void {
    if (value == 0) {
        put_char('0');
        return;
    }
    var buf: [20]u8 = undefined;
    var len: usize = 0;
    var v = value;
    while (v > 0) {
        buf[len] = @intCast((v % 10) + '0');
        len += 1;
        v /= 10;
    }
    // Print in reverse (most significant digit first)
    while (len > 0) {
        len -= 1;
        put_char(buf[len]);
    }
}

/// Set foreground/background color for subsequent writes.
pub fn set_color(fg: Color, bg: Color) void {
    default_attr = make_attr(fg, bg);
}

/// Move cursor to an absolute position.
pub fn set_cursor(col: usize, row: usize) void {
    cursor_col = if (col < VGA_WIDTH) col else VGA_WIDTH - 1;
    cursor_row = if (row < VGA_HEIGHT) row else VGA_HEIGHT - 1;
}

// ---------------------------------------------------------------------------
// C-ABI export for Rust FFI (called by kprint! macro in kernel/src/lib.rs)
// ---------------------------------------------------------------------------

/// Print a byte slice to the VGA buffer.
/// Called from Rust via: extern "C" fn vga_print_bytes(ptr: *const u8, len: usize)
pub export fn vga_print_bytes(ptr: [*]const u8, len: usize) callconv(.c) void {
    var i: usize = 0;
    while (i < len) : (i += 1) {
        put_char(ptr[i]);
    }
}
