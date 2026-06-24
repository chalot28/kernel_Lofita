#!/bin/bash
# create_ext2_disk.sh
# Creates a 10MB Ext2 disk image and puts some files in it

set -e

echo "[1/4] Creating 10MB raw disk image..."
dd if=/dev/zero of=disk.img bs=1M count=10 status=none

echo "[2/4] Formatting disk.img as Ext2..."
mkfs.ext2 -q -F disk.img

echo "[3/4] Mounting disk.img to temporary directory..."
mkdir -p mnt_ext2
sudo mount -o loop disk.img mnt_ext2

echo "[4/4] Creating dummy files..."
sudo bash -c 'echo "Hello from Ext2!" > mnt_ext2/hello_lofita.txt'
sudo bash -c 'echo "This is a secret file stored on the physical hard drive." > mnt_ext2/secret.txt'
sudo mkdir -p mnt_ext2/logs
sudo bash -c 'echo "System boot ok" > mnt_ext2/logs/boot.log'

echo "Unmounting..."
sudo umount mnt_ext2
rmdir mnt_ext2

echo "Done! disk.img is ready."
