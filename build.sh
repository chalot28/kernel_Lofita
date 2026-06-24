#!/bin/bash
set -e

ROOT_DIR=$(pwd)
RUST_DIR="$ROOT_DIR/kernel"
ZIG_DIR="$ROOT_DIR"

echo "=== 1. Building Monolithic Rust Core Subsystem ==="
cd "$RUST_DIR"
cargo build --release

echo "=== 2. Building Monolithic Zig Boot & Memory System ==="
cd "$ZIG_DIR"
zig build -Doptimize=ReleaseSafe

echo "=== 3. Monolithic Build Successful ==="
echo "Monolithic executable located at: $ZIG_DIR/zig-out/bin/lorifa_monolithic_kernel"
