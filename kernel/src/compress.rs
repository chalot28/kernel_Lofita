// kernel/src/compress.rs
// Written in Rust
// Lossless Run-Length Encoding (RLE) compression engine for initramfs / zram.

pub fn compress(src: &[u8]) -> Vec<u8> {
    if src.is_empty() {
        return Vec::new();
    }

    let mut dest = Vec::new();
    let mut i = 0;

    while i < src.len() {
        let mut run_len = 1;
        while i + run_len < src.len() && src[i + run_len] == src[i] && run_len < 255 {
            run_len += 1;
        }
        dest.push(run_len as u8);
        dest.push(src[i]);
        i += run_len;
    }

    dest
}

pub fn decompress(src: &[u8]) -> Result<Vec<u8>, &'static str> {
    if src.len() % 2 != 0 {
        return Err("Malformed compressed data: must be pairs of count and byte");
    }

    let mut dest = Vec::new();
    let mut i = 0;

    while i < src.len() {
        let count = src[i] as usize;
        let byte = src[i + 1];
        
        for _ in 0..count {
            dest.push(byte);
        }
        i += 2;
    }

    Ok(dest)
}

// FFI bindings for Zig loader

#[no_mangle]
pub extern "C" fn rust_compress_data(
    src_ptr: *const u8,
    src_len: usize,
    dst_ptr: *mut u8,
    dst_max_len: usize,
) -> usize {
    let src = unsafe { std::slice::from_raw_parts(src_ptr, src_len) };
    let compressed = compress(src);
    if compressed.len() > dst_max_len {
        return 0; // buffer too small
    }
    unsafe {
        std::ptr::copy_nonoverlapping(compressed.as_ptr(), dst_ptr, compressed.len());
    }
    compressed.len()
}

#[no_mangle]
pub extern "C" fn rust_decompress_data(
    src_ptr: *const u8,
    src_len: usize,
    dst_ptr: *mut u8,
    dst_max_len: usize,
) -> usize {
    let src = unsafe { std::slice::from_raw_parts(src_ptr, src_len) };
    match decompress(src) {
        Ok(decompressed) => {
            if decompressed.len() > dst_max_len {
                return 0;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(decompressed.as_ptr(), dst_ptr, decompressed.len());
            }
            decompressed.len()
        }
        Err(_) => 0,
    }
}
