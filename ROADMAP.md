# Lofita OS: Future Development Roadmap

Để nâng cấp Lofita từ một "Educational Kernel" (Kernel giáo dục/thử nghiệm) trở thành một hệ điều hành thực thụ có khả năng sử dụng trong thực tế, dưới đây là danh sách toàn diện các vấn đề và tính năng cần được lên kế hoạch bổ sung (chia theo từng hệ thống phụ).

## 1. Quản lý Bộ nhớ (Memory Management)
- **Swapping (Demand Paging xuống đĩa):** Hiện tại hệ thống sẽ bị treo hoặc crash nếu hết RAM. Cần cơ chế đẩy bớt các trang nhớ ít dùng (inactive pages) xuống ổ cứng (Swap file/partition) và nạp lại khi cần.
- **Copy-on-Write (CoW):** Rất quan trọng để tối ưu hóa hiệu năng cho hàm `fork()`. Thay vì copy toàn bộ RAM của tiến trình cha sang tiến trình con, OS chỉ copy khi có một bên thực hiện lệnh "Ghi" (Write).
- **Page Cache / Buffer Cache:** Lưu trữ đệm các block đọc từ ổ cứng lên RAM để các lần đọc sau diễn ra tức thời, không cần truy xuất đĩa vật lý chậm chạp.
- **Thu hồi bộ nhớ (OOM Killer):** Cơ chế theo dõi và tiêu diệt tiến trình ngốn RAM khi hệ thống cạn kiệt tài nguyên.

## 2. Quản lý Tiến trình & Tương tác (Process & IPC)
- **Shared Memory & Futex:** Khả năng cho phép 2 tiến trình chia sẻ chung một vùng RAM ảo để truyền dữ liệu tốc độ cực cao, kết hợp với futex để đồng bộ hóa (mutex/semaphore ở không gian User).
- **Inter-Process Communication (IPC):** Hỗ trợ đầy đủ Pipes (đường ống: `ls | grep`), Message Queues, và Unix Domain Sockets.
- **Cơ chế Signals:** Xử lý các ngắt phần mềm gửi đến ứng dụng (ví dụ Ctrl+C sinh ra tín hiệu `SIGINT`, tiến trình có thể đăng ký hàm handler hoặc bị kill).
- **Multithreading (Đa luồng):** Hỗ trợ Syscall `clone()` để một tiến trình có thể tạo ra nhiều luồng chia sẻ chung không gian bộ nhớ ảo (cần thiết để port pthreads).

## 3. Hệ thống Tệp & Lưu trữ (Storage & VFS)
- **VFS Mở rộng:** Bổ sung các hệ thống tệp ảo để tương tác với Kernel từ User-space, bao gồm:
  - `/dev`: Quản lý Device Files (ví dụ `/dev/sda`, `/dev/tty`).
  - `/proc` & `/sys`: Đọc thông tin phần cứng, RAM, tiến trình (ví dụ `/proc/cpuinfo`, `/proc/meminfo`).
- **Ghi đĩa (Write Support) & Journaling:** Xây dựng thuật toán cấp phát block/inode mới cho Ext2. Nâng cấp lên Ext3/Ext4 để có tính năng ghi nhật ký (Journaling), chống mất dữ liệu khi mất điện đột ngột.
- **Dynamic Linker (.so files):** Hiện tại Lofita chỉ chạy được các file ELF build tĩnh (Statically linked). Để tiết kiệm RAM, cần hỗ trợ Shared Libraries (.so) và Dynamic Linking.

## 4. Tương tác Phần cứng (Hardware & Drivers)
- **PCI / PCIe Enumeration:** Kernel tự động quét bus PCI để phát hiện xem máy tính đang cắm những phần cứng gì (Card mạng nào, Card hình gì...) và nạp driver tương ứng.
- **APIC & IOAPIC:** Thay thế chip ngắt PIC cũ kỹ từ thời MS-DOS bằng APIC hiện đại. Đây là điều kiện bắt buộc để làm được hệ thống Đa nhân (SMP - Multi-core).
- **Direct Memory Access (DMA):** Cho phép ổ cứng hoặc card mạng đẩy thẳng dữ liệu vào RAM mà không bắt CPU phải làm nhiệm vụ trung gian (hiện tại chúng ta dùng PIO rất tốn CPU).
- **ACPI (Power Management):** Quản lý năng lượng, cho phép OS ra lệnh tắt máy (Shutdown), khởi động lại mềm, hoặc chuyển sang chế độ ngủ (Sleep/Suspend).
- **Đồ họa Framebuffer (GUI Base):** Chuyển từ VGA Text Mode (chỉ vẽ được chữ) sang VBE/GOP Framebuffer (vẽ được pixel, render hình ảnh, font chữ tùy chỉnh, hiển thị cửa sổ).

## 5. Mạng (Networking Stack)
- **Kiến trúc Socket API:** Chuẩn POSIX.
- **Triển khai toàn bộ ngăn xếp giao thức:** Ethernet -> ARP -> IPv4/IPv6 -> ICMP (để ping) -> UDP / TCP.
- **DHCP:** Tự động nhận IP qua giao thức DHCP.

## 6. Môi trường Userland & C Library (libc)
- **Port libc:** Hệ điều hành không chỉ có Kernel. Để người dùng xài được, cần phải "Port" một bộ thư viện C chuẩn (như musl hoặc newlib) sang nền tảng Lofita.
- **Port bộ công cụ Coreutils:** Cần biên dịch được các lệnh cơ bản như `ls`, `cat`, `mkdir`, `rm`, `vi`/`nano` để chạy trên Kernel của chúng ta.

## 7. Bảo mật & Ổn định (Security)
- **Phân quyền người dùng (Users/Groups):** Triển khai khái niệm UID/GID, kiểm tra quyền R/W/X khi mở file.
- **ASLR (Address Space Layout Randomization):** Ngẫu nhiên hóa địa chỉ load của stack/heap/code mỗi lần chạy để chống các cuộc tấn công tràn bộ đệm (Buffer Overflow).
