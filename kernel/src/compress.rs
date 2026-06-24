// kernel/src/compress.rs
// Written in Rust (no_std)
// Lossless Run-Length Encoding (RLE) compression engine for initramfs / zram.
//
// no_std changes:
//   - Vec             → alloc::vec::Vec
//   - std::slice::from_raw_parts / std::ptr::copy_nonoverlapping
//                     → core::slice::from_raw_parts / core::ptr::copy_nonoverlapping

use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// RLE compress
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// RLE decompress
// ---------------------------------------------------------------------------

pub fn decompress(src: &[u8]) -> Result<Vec<u8>, &'static str> {
    if src.len() % 2 != 0 {
        return Err("Malformed RLE data: must be count+byte pairs");
    }

    let mut dest = Vec::new();
    let mut i = 0;

    while i < src.len() {
        let count = src[i] as usize;
        let byte  = src[i + 1];
        for _ in 0..count {
            dest.push(byte);
        }
        i += 2;
    }

    Ok(dest)
}

// ---------------------------------------------------------------------------
// C-ABI FFI exports (for Zig loader)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn rust_compress_data(
    src_ptr:     *const u8,
    src_len:     usize,
    dst_ptr:     *mut u8,
    dst_max_len: usize,
) -> usize {
    let src = unsafe { core::slice::from_raw_parts(src_ptr, src_len) };
    let compressed = compress(src);
    if compressed.len() > dst_max_len {
        return 0;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(compressed.as_ptr(), dst_ptr, compressed.len());
    }
    compressed.len()
}

#[no_mangle]
pub extern "C" fn rust_decompress_data(
    src_ptr:     *const u8,
    src_len:     usize,
    dst_ptr:     *mut u8,
    dst_max_len: usize,
) -> usize {
    let src = unsafe { core::slice::from_raw_parts(src_ptr, src_len) };
    match decompress(src) {
        Ok(decompressed) => {
            if decompressed.len() > dst_max_len {
                return 0;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(decompressed.as_ptr(), dst_ptr, decompressed.len());
            }
            decompressed.len()
        }
        Err(_) => 0,
    }
}
