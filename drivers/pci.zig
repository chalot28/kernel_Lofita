const io = @import("../arch/x86_64/io.zig");
const vga = @import("vga.zig");

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

pub fn pci_config_read_dword(bus: u8, slot: u8, func: u8, offset: u8) u32 {
    const lbus = @as(u32, bus);
    const lslot = @as(u32, slot);
    const lfunc = @as(u32, func);
    
    const address: u32 = (lbus << 16) | (lslot << 11) | (lfunc << 8) | (offset & 0xFC) | 0x80000000;
    
    io.outl(CONFIG_ADDRESS, address);
    return io.inl(CONFIG_DATA);
}

pub fn pci_config_read_word(bus: u8, slot: u8, func: u8, offset: u8) u16 {
    const dword = pci_config_read_dword(bus, slot, func, offset);
    return @as(u16, @truncate(dword >> @as(u5, @intCast((offset & 2) * 8))));
}

pub fn get_vendor_id(bus: u8, slot: u8, func: u8) u16 {
    return pci_config_read_word(bus, slot, func, 0);
}

pub fn get_device_id(bus: u8, slot: u8, func: u8) u16 {
    return pci_config_read_word(bus, slot, func, 2);
}

pub fn get_class_id(bus: u8, slot: u8, func: u8) u8 {
    const r = pci_config_read_word(bus, slot, func, 0x0A);
    return @as(u8, @truncate(r >> 8));
}

pub fn get_subclass_id(bus: u8, slot: u8, func: u8) u8 {
    const r = pci_config_read_word(bus, slot, func, 0x0A);
    return @as(u8, @truncate(r & 0xFF));
}

pub fn get_header_type(bus: u8, slot: u8, func: u8) u8 {
    const r = pci_config_read_word(bus, slot, func, 0x0E);
    return @as(u8, @truncate(r & 0xFF));
}

pub fn check_function(bus: u8, slot: u8, func: u8) void {
    const vendor_id = get_vendor_id(bus, slot, func);
    if (vendor_id == 0xFFFF) return;
    
    const device_id = get_device_id(bus, slot, func);
    const class_id = get_class_id(bus, slot, func);
    const subclass_id = get_subclass_id(bus, slot, func);
    
    vga.print("  * Bus ");
    vga.print_dec(bus);
    vga.print(", Slot ");
    vga.print_dec(slot);
    vga.print(", Func ");
    vga.print_dec(func);
    vga.print(" | Vendor: ");
    vga.print_hex(vendor_id);
    vga.print(", Device: ");
    vga.print_hex(device_id);
    vga.print(", Class: ");
    vga.print_hex(class_id);
    vga.print(":");
    vga.print_hex(subclass_id);
    vga.print("\n");
}

pub fn check_device(bus: u8, slot: u8) void {
    const vendor_id = get_vendor_id(bus, slot, 0);
    if (vendor_id == 0xFFFF) return; // Device doesn't exist
    
    check_function(bus, slot, 0);
    
    const header_type = get_header_type(bus, slot, 0);
    if ((header_type & 0x80) != 0) {
        // It's a multi-function device, so check remaining functions
        var func: u8 = 1;
        while (func < 8) : (func += 1) {
            check_function(bus, slot, func);
        }
    }
}

pub fn pci_init() void {
    vga.print("[PCI] Enumerating PCI Bus...\n");
    var bus: u16 = 0;
    while (bus < 256) : (bus += 1) {
        var slot: u8 = 0;
        while (slot < 32) : (slot += 1) {
            check_device(@as(u8, @truncate(bus)), slot);
        }
    }
    vga.print("[PCI] Enumeration complete.\n");
}
