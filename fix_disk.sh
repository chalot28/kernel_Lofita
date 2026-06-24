echo 'Formatting disk.img...'; dd if=/dev/zero of=disk.img bs=1M count=10 status=none; mkfs.ext2 -q -F disk.img; echo 'Done! Now run ./build.sh run'
