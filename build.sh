#!/usr/bin/env bash
# build.sh — Lofita OS Full Build Script
# Compiles Rust core (no_std), then Zig kernel, then assembles a bootable ISO.
#
# Prerequisites:
#   zig         >= 0.12
#   rust nightly with target x86_64-unknown-none
#   grub-mkrescue + xorriso
#   qemu-system-x86_64 (optional, for 'run' target)
#
# Usage:
#   ./build.sh          # compile kernel ELF only
#   ./build.sh iso      # also create lofita.iso
#   ./build.sh run      # iso + launch QEMU
#   ./build.sh clean    # remove build artifacts

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

KERNEL_DIR="kernel"
RUST_TARGET="x86_64-unknown-none"
ISO_DIR="zig-out/iso"
ISO_OUT="lofita.iso"
QEMU="qemu-system-x86_64"

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------

info()  { echo -e "\033[1;34m[BUILD]\033[0m $*"; }
ok()    { echo -e "\033[1;32m[OK]\033[0m    $*"; }
err()   { echo -e "\033[1;31m[ERROR]\033[0m $*" >&2; exit 1; }
warn()  { echo -e "\033[1;33m[WARN]\033[0m  $*"; }

check_tool() {
    command -v "$1" &>/dev/null || err "Required tool '$1' not found. Please install it."
}

# ---------------------------------------------------------------------------
# Step 1: Build Rust kernel core (no_std)
# ---------------------------------------------------------------------------

build_rust() {
    info "Building Rust kernel core (target: ${RUST_TARGET}) ..."
    check_tool cargo
    check_tool rustup

    # Ensure nightly toolchain and the bare-metal target are installed
    if ! rustup target list --installed | grep -q "${RUST_TARGET}"; then
        warn "Target '${RUST_TARGET}' not installed. Installing via rustup..."
        rustup target add "${RUST_TARGET}"
    fi

    cd "${KERNEL_DIR}"
    cargo build --target "${RUST_TARGET}" --release 2>&1
    cd ..
    ok "Rust core built → ${KERNEL_DIR}/target/${RUST_TARGET}/release/liblorifa_kernel_core.a"
}

# ---------------------------------------------------------------------------
# Step 2: Build Zig kernel (freestanding x86_64)
# ---------------------------------------------------------------------------

build_zig() {
    info "Building Zig kernel (freestanding x86_64) ..."
    check_tool zig
    zig build --release=safe 2>&1
    ok "Zig kernel built → zig-out/bin/lorifa_kernel"
}

# ---------------------------------------------------------------------------
# Step 3: Assemble bootable ISO
# ---------------------------------------------------------------------------

build_iso() {
    info "Assembling bootable ISO ..."
    check_tool grub-mkrescue
    check_tool xorriso

    mkdir -p "${ISO_DIR}/boot/grub"
    cp zig-out/bin/lorifa_kernel "${ISO_DIR}/boot/lorifa_kernel.elf"
    cp iso/boot/grub/grub.cfg   "${ISO_DIR}/boot/grub/grub.cfg"

    grub-mkrescue -o "${ISO_OUT}" "${ISO_DIR}" 2>&1
    ok "ISO image created → ${ISO_OUT}"
}

# ---------------------------------------------------------------------------
# Step 4: Launch QEMU
# ---------------------------------------------------------------------------

run_qemu() {
    info "Launching Lofita OS in QEMU ..."
    check_tool "${QEMU}"
    ${QEMU} \
        -cdrom     "${ISO_OUT}" \
        -drive     file=disk.img,format=raw,index=0,media=disk \
        -m         256M \
        -serial    stdio \
        -display   sdl \
        -no-reboot \
        -no-shutdown
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

clean() {
    info "Cleaning build artifacts ..."
    rm -rf zig-out zig-cache .zig-cache "${ISO_OUT}" "${ISO_DIR}"
    cd "${KERNEL_DIR}" && cargo clean && cd ..
    ok "Clean complete."
}

ACTION="${1:-build}"

case "${ACTION}" in
    build)
        build_rust
        build_zig
        ok "Kernel build complete."
        ;;
    iso)
        build_rust
        build_zig
        build_iso
        ;;
    run)
        build_rust
        build_zig
        build_iso
        run_qemu
        ;;
    clean)
        clean
        ;;
    *)
        echo "Usage: $0 [build|iso|run|clean]"
        exit 1
        ;;
esac
