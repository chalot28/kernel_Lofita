// build.zig — Lofita OS Build System
// Compiles a freestanding x86_64 kernel ELF, then builds a bootable ISO.
//
// Usage:
//   zig build              → compile kernel ELF only
//   zig build iso          → compile + create lofita.iso (requires grub-mkrescue)
//   zig build run          → build ISO and launch in QEMU

const std = @import("std");

pub fn build(b: *std.Build) void {
    // -----------------------------------------------------------------------
    // Target: x86_64 freestanding (no OS, no libc)
    // -----------------------------------------------------------------------
    const target = b.resolveTargetQuery(.{
        .cpu_arch = .x86_64,
        .os_tag = .freestanding,
        .abi = .none,
    });

    const optimize = b.standardOptimizeOption(.{});

    // -----------------------------------------------------------------------
    // Kernel executable (ELF)
    // -----------------------------------------------------------------------
    const kernel_mod = b.createModule(.{
        .root_source_file = b.path("main.zig"),
        .target = target,
        .optimize = optimize,
        .strip = false,
    });
    kernel_mod.red_zone = false;

    const kernel = b.addExecutable(.{
        .name = "lorifa_kernel",
        .root_module = kernel_mod,
    });
    kernel_mod.addAssemblyFile(b.path("boot/multiboot.s"));

    // Use our custom linker script to place the kernel at 0x100000
    kernel.setLinkerScript(b.path("kernel.ld"));

    // Link the Rust core static library (must be compiled separately with
    // `cargo build --target x86_64-unknown-none --release` inside kernel/)
    kernel_mod.addLibraryPath(b.path("kernel/target/x86_64-unknown-none/release"));
    kernel_mod.linkSystemLibrary("lorifa_kernel_core", .{});

    // Install the ELF into zig-out/bin/
    b.installArtifact(kernel);

    // -----------------------------------------------------------------------
    // ISO build step: assemble GRUB bootable image
    //   Requires: grub-mkrescue, xorriso
    // -----------------------------------------------------------------------
    const iso_dir = "zig-out/iso";

    // Step: create directory layout for grub-mkrescue
    const mkdir_cmd = b.addSystemCommand(&.{
        "sh", "-c",
        "mkdir -p " ++ iso_dir ++ "/boot/grub && " ++
            "cp zig-out/bin/lorifa_kernel " ++ iso_dir ++ "/boot/lorifa_kernel.elf && " ++
            "cp iso/boot/grub/grub.cfg " ++ iso_dir ++ "/boot/grub/grub.cfg",
    });
    mkdir_cmd.step.dependOn(b.getInstallStep());

    // Step: run grub-mkrescue
    const grub_cmd = b.addSystemCommand(&.{
        "grub-mkrescue", "-o", "lofita.iso", iso_dir,
    });
    grub_cmd.step.dependOn(&mkdir_cmd.step);

    const iso_step = b.step("iso", "Build a bootable ISO image (requires grub-mkrescue)");
    iso_step.dependOn(&grub_cmd.step);

    // -----------------------------------------------------------------------
    // Run step: launch QEMU with the ISO
    //   Requires: qemu-system-x86_64
    // -----------------------------------------------------------------------
    const qemu_cmd = b.addSystemCommand(&.{
        "qemu-system-x86_64",
        "-cdrom",
        "lofita.iso",
        "-m",
        "256M",
        "-serial", "stdio", // Serial output goes to terminal
        "-display",   "sdl", // VGA window (change to "none" for headless)
        "-no-reboot",
    });
    qemu_cmd.step.dependOn(&grub_cmd.step);

    const run_step = b.step("run", "Build ISO and launch Lofita OS in QEMU");
    run_step.dependOn(&qemu_cmd.step);
}
