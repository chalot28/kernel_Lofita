// kernel/src/web.rs
// Written in Rust (no_std)
// WebKit WebView Engine integration (bare-metal stub).
//
// no_std changes:
//   - String                        → alloc::string::String
//   - println!                      → kprint!
//   - std::slice::from_raw_parts    → core::slice::from_raw_parts
//   - std::str::from_utf8           → core::str::from_utf8

use alloc::string::{String, ToString};
use crate::kprint;

pub struct WebView {
    pub url:             String,
    pub width:           u32,
    pub height:          u32,
    pub fb_virtual_addr: usize,
}

impl WebView {
    pub fn new(url: &str) -> Self {
        WebView {
            url: url.to_string(),
            width:  1024,
            height: 768,
            fb_virtual_addr: 0,
        }
    }

    /// Request a framebuffer from the Virtual Address Space Manager.
    pub fn allocate_framebuffer(&mut self, has_mem_alloc: bool) -> Result<usize, &'static str> {
        if !has_mem_alloc {
            return Err("WebKit: Lacks MEM_ALLOC capability.");
        }
        let size          = (self.width * self.height * 4) as usize;
        let mock_fb_addr  = 0xD000_0000usize;
        self.fb_virtual_addr = mock_fb_addr;
        kprint!("[WebKit] Framebuffer: {} bytes at 0x{:x}\n", size, mock_fb_addr);
        Ok(mock_fb_addr)
    }

    /// Render stub — draws to VGA text buffer.
    pub fn render(&self) {
        kprint!("[WebKit] Rendering: {}\n", self.url);
        kprint!("+-----------------------------------+\n");
        kprint!("| Lofita Web Browser                |\n");
        if self.url.contains("lorifa.org") {
            kprint!("| Welcome to Lofita OS Project      |\n");
        } else {
            kprint!("| Loading: {}    |\n", self.url);
        }
        kprint!("+-----------------------------------+\n");
    }
}

// ---------------------------------------------------------------------------
// C-ABI FFI export
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn rust_webkit_render(
    url_ptr:       *const u8,
    url_len:       usize,
    has_mem_alloc: u32,
) -> i32 {
    let url_slice = unsafe { core::slice::from_raw_parts(url_ptr, url_len) };
    let url       = core::str::from_utf8(url_slice).unwrap_or("about:blank");

    let mut webview = WebView::new(url);
    if webview.allocate_framebuffer(has_mem_alloc != 0).is_err() {
        return -1;
    }
    webview.render();
    0
}
