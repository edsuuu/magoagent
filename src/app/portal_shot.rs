//! A picture of a portal's page for the agents: what the page shows now, as a PNG.

use crate::prelude::*;

/// BGRA pixels as GDI gives them (its alpha left at zero) into opaque RGBA.
fn opaque_rgba(mut pixels: Vec<u8>) -> Vec<u8> {
    for p in pixels.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        p[3] = 255;
    }
    pixels
}

/// The picture as a PNG in base64, as the MCP sends images.
fn png_base64(width: u32, height: u32, rgba: Vec<u8>) -> Result<String, String> {
    let image = image::RgbaImage::from_raw(width, height, rgba).ok_or("imagem vazia")?;
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(crate::render::base64(&png))
}

/// What a window shows, as BGRA: the system draws it into a bitmap (PrintWindow), the parts under
/// other windows too. None when it has no size or the system could not draw it (a hidden window).
#[cfg(windows)]
fn window_pixels(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_per_meter: i32,
        y_per_meter: i32,
        colors_used: u32,
        colors_important: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetClientRect(hwnd: isize, rect: *mut [i32; 4]) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
        fn PrintWindow(hwnd: isize, dc: isize, flags: u32) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: isize) -> isize;
        fn CreateCompatibleBitmap(dc: isize, width: i32, height: i32) -> isize;
        fn SelectObject(dc: isize, object: isize) -> isize;
        fn DeleteObject(object: isize) -> i32;
        fn DeleteDC(dc: isize) -> i32;
        fn GetDIBits(dc: isize, bitmap: isize, start: u32, lines: u32, bits: *mut u8, info: *mut BitmapInfoHeader, usage: u32) -> i32;
    }
    // PW_CLIENTONLY | PW_RENDERFULLCONTENT: the page is drawn by the GPU, which a plain PrintWindow leaves black
    const FLAGS: u32 = 1 | 2;
    // SAFETY: plain GDI handles, made and freed here; the buffer has the size GetDIBits is told
    unsafe {
        let mut rect = [0i32; 4];
        if GetClientRect(hwnd, &mut rect) == 0 {
            return None;
        }
        let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
        if w <= 0 || h <= 0 {
            return None;
        }
        let screen = GetDC(hwnd);
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let before = SelectObject(dc, bitmap);
        let drawn = PrintWindow(hwnd, dc, FLAGS) != 0;
        // GetDIBits wants the bitmap out of the device context
        SelectObject(dc, before);
        // top-down rows (a negative height), 32 bits a pixel, no compression
        let mut info = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: w,
            height: -h,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_per_meter: 0,
            y_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        };
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        let read = drawn && GetDIBits(dc, bitmap, 0, h as u32, pixels.as_mut_ptr(), &mut info, 0) != 0;
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(hwnd, screen);
        read.then_some((w as u32, h as u32, pixels))
    }
}

impl DrawingApp {
    /// The agents' `portal_screenshot`: the open tab of portal `id` as a PNG in base64.
    pub(crate) fn portal_shot(&self, id: &str) -> Result<String, String> {
        let (width, height, rgba) = self.page_rgba(id)?;
        png_base64(width, height, rgba)
    }

    /// The open tab's picture as opaque RGBA (also what Copiar Captura de Tela puts on the clipboard).
    // ponytail: Windows only (the Mac has `design::snapshot`, which answers later; wire it when the Mac's turn comes)
    pub(crate) fn page_rgba(&self, id: &str) -> Result<(u32, u32, Vec<u8>), String> {
        #[cfg(windows)]
        {
            use wry::WebViewExtWindows;
            let tab = self.open_tab_view(id).ok_or("a página desse portal não está aberta")?;
            let (width, height, pixels) =
                window_pixels(tab.webview.hwnd().0 as isize).ok_or("não foi possível capturar a página: ela precisa estar na tela, com o portal visível")?;
            Ok((width, height, opaque_rgba(pixels)))
        }
        #[cfg(not(windows))]
        {
            let _ = id;
            Err("a captura da página só existe no Windows por enquanto; use portal_read ou o screenshot do app".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_systems_pixels_become_an_opaque_png() {
        // blue, green, red, alpha as GDI leaves them: two pixels
        let rgba = opaque_rgba(vec![10, 20, 30, 0, 200, 100, 50, 0]);
        assert_eq!(rgba, [30, 20, 10, 255, 50, 100, 200, 255]);
        let png = png_base64(2, 1, rgba).unwrap();
        // "iVBORw0KGgo" is a PNG's signature in base64
        assert!(png.starts_with("iVBORw0KGgo"), "{png}");
        assert!(png_base64(2, 2, vec![0; 4]).is_err(), "pixels that don't fill the size");
    }
}
