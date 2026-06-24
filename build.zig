const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const exe = b.addExecutable(.{
        .name = "lorifa_monolithic_kernel",
        .root_source_file = .{ .path = "init/main.zig" },
        .target = target,
        .optimize = optimize,
    });

    // Link the Rust core static library compiled in the kernel/ directory
    exe.addLibraryPath(.{ .path = "kernel/target/release" });
    exe.linkSystemLibrary("lorifa_kernel_core");
    exe.linkLibC();

    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    const run_step = b.step("run", "Run the monolithic Lorifa kernel");
    run_step.dependOn(&run_cmd.step);
}
