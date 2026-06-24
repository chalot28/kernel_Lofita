// compat/linux/src/main.rs
// Written in Rust
// Entrypoint for wine-host compatibility translation service daemon.

mod syscall;

use syscall::SyscallRouter;

fn main() {
    println!("=== [compat/linux] lorifa-wine-host daemon starting ===");
    let mut router = SyscallRouter::new();

    // Mock trace
    let result = router.route(1, [1, 0x40000000, 14, 0, 0, 0], 600, 0x1FF);
    println!("  -> stdout test status: {}", result);

    println!("=== [compat/linux] lorifa-wine-host daemon closed ===");
}
