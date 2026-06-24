// main.zig — Thin root wrapper to establish the project root as the module root.
// All actual kernel code lives in init/main.zig and its imports.
// The comptime import below ensures the boot stub (which provides _start) and
// all other Zig source files are linked into the final kernel ELF.
comptime {
    _ = @import("init/main.zig");
}
