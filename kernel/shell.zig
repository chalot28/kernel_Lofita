// kernel/shell.zig - Lofita OS Interactive Terminal Shell
// Written in Zig (freestanding - no std, no libc)
//
// A simple command-line shell that reads keyboard input and
// displays output via the VGA text-mode driver.
// Runs in the kernel main loop after all subsystems are initialized.

const vga = @import("../drivers/vga.zig");
const kb = @import("../drivers/keyboard.zig");
const ppa = @import("../mm/ppa.zig");

// ---------------------------------------------------------------------------
// Line-editing buffer
// ---------------------------------------------------------------------------

const LINE_BUF_SIZE = 256;

// ---------------------------------------------------------------------------
// Command dispatch
// ---------------------------------------------------------------------------

fn print_prompt() void {
    vga.set_color(.LightGreen, .Black);
    vga.print("lorifa$ ");
    vga.set_color(.White, .Black);
}

fn cmd_help() void {
    vga.set_color(.Cyan, .Black);
    vga.print("Available commands:\n");
    vga.print("  help                  Show this help message\n");
    vga.print("  clear                 Clear the screen\n");
    vga.print("  status                Show memory and kernel status\n");
    vga.print("  echo <text>           Echo text back to the terminal\n");
    vga.print("  reboot                Halt the system\n");
    vga.set_color(.White, .Black);
}

fn cmd_status() void {
    const free_pgs = ppa.free_pages();
    const total_pgs = ppa.TOTAL_PAGES;
    const used_pgs = total_pgs - free_pgs;
    const total_mb = @as(u64, total_pgs * ppa.PAGE_SIZE) / (1024 * 1024);
    const used_mb = @as(u64, used_pgs * ppa.PAGE_SIZE) / (1024 * 1024);
    const free_mb = @as(u64, free_pgs * ppa.PAGE_SIZE) / (1024 * 1024);

    vga.set_color(.Yellow, .Black);
    vga.print("--- Lofita Kernel Status ---\n");
    vga.set_color(.White, .Black);
    vga.print("  Total Memory: ");
    vga.print_dec(total_mb);
    vga.print(" MB (");
    vga.print_dec(total_pgs);
    vga.print(" pages)\n");

    vga.print("  Used Memory:  ");
    vga.print_dec(used_mb);
    vga.print(" MB (");
    vga.print_dec(used_pgs);
    vga.print(" pages)\n");

    vga.print("  Free Memory:  ");
    vga.print_dec(free_mb);
    vga.print(" MB (");
    vga.print_dec(free_pgs);
    vga.print(" pages)\n");

    vga.print("  VGA: 80x25 text mode @ 0xB8000\n");
    vga.print("  Architecture: x86_64, Long Mode\n");
}

fn cmd_echo(args: []const u8) void {
    vga.print(args);
    vga.print("\n");
}

// ---------------------------------------------------------------------------
// Simple command tokenizer
// ---------------------------------------------------------------------------

/// Parse a line into command and arguments, then dispatch.
fn execute_line(line: []const u8) void {
    // Find the end of the first word (command)
    var cmd_end: usize = 0;
    while (cmd_end < line.len and line[cmd_end] != ' ' and line[cmd_end] != '\t') : (cmd_end += 1) {}

    const cmd = line[0..cmd_end];

    // Skip whitespace to find argument start
    var arg_start: usize = cmd_end;
    while (arg_start < line.len and (line[arg_start] == ' ' or line[arg_start] == '\t')) : (arg_start += 1) {}

    const args = if (arg_start < line.len) line[arg_start..] else "";

    if (cmd.len == 0) return;

    // Command dispatch
    if (eq(cmd, "help")) {
        cmd_help();
    } else if (eq(cmd, "clear")) {
        vga.clear();
    } else if (eq(cmd, "status")) {
        cmd_status();
    } else if (eq(cmd, "echo")) {
        cmd_echo(args);
    } else if (eq(cmd, "reboot") or eq(cmd, "exit")) {
        vga.print("System halted. Press Ctrl+Alt+Del or reset.\n");
        while (true) {
            asm volatile ("cli; hlt");
        }
    } else {
        vga.set_color(.LightRed, .Black);
        vga.print("Unknown command: ");
        vga.print(cmd);
        vga.print("\n");
        vga.set_color(.White, .Black);
        vga.print("Type 'help' for available commands.\n");
    }
}

/// Case-sensitive string equality for slices.
fn eq(a: []const u8, b: []const u8) bool {
    if (a.len != b.len) return false;
    for (a, b) |ca, cb| {
        if (ca != cb) return false;
    }
    return true;
}

// ---------------------------------------------------------------------------
// Main shell loop
// ---------------------------------------------------------------------------

/// Enter the interactive shell. Never returns.
pub fn shell_main() noreturn {
    vga.set_color(.LightGreen, .Black);
    vga.print("\n");
    vga.print("==================================================\n");
    vga.print("      Lofita Kernel - Interactive Terminal        \n");
    vga.print("      Type 'help' for available commands          \n");
    vga.print("==================================================\n");
    vga.set_color(.White, .Black);

    print_prompt();

    // Line input buffer
    var line_buf: [LINE_BUF_SIZE]u8 = undefined;
    var line_len: usize = 0;

    while (true) {
        // Check for keyboard input
        if (kb.read_key()) |ch| {
            switch (ch) {
                '\n' => {
                    vga.put_char('\n');
                    // Execute the command
                    const line = line_buf[0..line_len];
                    execute_line(line);
                    line_len = 0;
                    print_prompt();
                },
                0x08 => { // Backspace
                    if (line_len > 0) {
                        line_len -= 1;
                        vga.put_char(0x08); // Move cursor back
                        vga.put_char(' '); // Erase character
                        vga.put_char(0x08); // Move cursor back again
                    }
                },
                '\t' => {
                    // Tab: ignore for now, could add autocomplete later
                },
                else => {
                    if (line_len < LINE_BUF_SIZE - 1) {
                        line_buf[line_len] = ch;
                        line_len += 1;
                        vga.put_char(ch);
                    }
                },
            }
        } else {
            // No key available - halt the CPU until next interrupt to save CPU cycles
            asm volatile ("hlt");
        }
    }
}
