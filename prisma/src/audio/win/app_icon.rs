//! Icono del programa que suena (sacado de su .exe), en píxeles RGBA

use windows::Win32::{
    Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits},
    UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO, PrivateExtractIconsW},
};

/// Lado del icono, en píxeles
const SIZE: i32 = 48;

/// Icono del programa (`spotify.exe`) mientras tenga sonido: lado y píxeles
/// RGBA
pub fn app_icon(process: &str) -> Option<(u32, Vec<u8>)> {
    let path = super::volume::process_path(process)?;
    let mut wide = [0u16; 260];
    for (i, c) in path.encode_utf16().take(259).enumerate() {
        wide[i] = c;
    }
    unsafe {
        let mut icons = [HICON::default()];
        let n = PrivateExtractIconsW(&wide, 0, SIZE, SIZE, Some(&mut icons), None, 0);
        if n == 0 || n == u32::MAX || icons[0].is_invalid() {
            return None;
        }
        let rgba = pixels(icons[0]);
        let _ = DestroyIcon(icons[0]);
        rgba.map(|px| (SIZE as u32, px))
    }
}

/// Píxeles RGBA del icono
unsafe fn pixels(icon: HICON) -> Option<Vec<u8>> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;
        let dc = CreateCompatibleDC(None);
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE, // de arriba abajo
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (SIZE * SIZE * 4) as usize];
        let rows = GetDIBits(dc, info.hbmColor, 0, SIZE as u32, Some(bgra.as_mut_ptr().cast()), &mut bmi, DIB_RGB_COLORS);
        let _ = DeleteDC(dc);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
        if rows == 0 {
            return None;
        }
        // Iconos viejos sin transparencia: todo opaco
        let no_alpha = bgra.chunks(4).all(|p| p[3] == 0);
        for p in bgra.chunks_mut(4) {
            p.swap(0, 2);
            if no_alpha {
                p[3] = 255;
            }
        }
        Some(bgra)
    }
}
