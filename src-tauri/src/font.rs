use crate::models::{FontFace, FontFormat};
use std::{fs, path::Path};

pub struct ParsedFont {
    pub format: FontFormat,
    pub faces: Vec<FontFace>,
}

pub fn validate_extension(path: &Path) -> Result<FontFormat, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "文件没有字体扩展名".to_string())?;

    match extension.as_str() {
        "ttf" => Ok(FontFormat::Ttf),
        "otf" => Ok(FontFormat::Otf),
        "ttc" => Ok(FontFormat::Ttc),
        _ => Err("仅支持 TTF、OTF 和 TTC 文件".to_string()),
    }
}

pub fn parse_font(path: &Path) -> Result<ParsedFont, String> {
    let format = validate_extension(path)?;
    let data = fs::read(path).map_err(|error| error.to_string())?;
    let face_count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    let fallback_family = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未知字体")
        .to_string();
    let mut faces = Vec::with_capacity(face_count as usize);

    for index in 0..face_count {
        let face = ttf_parser::Face::parse(&data, index)
            .map_err(|error| format!("无法解析字体面 {index}: {error:?}"))?;
        let family = find_name(&face, ttf_parser::name_id::FAMILY)
            .unwrap_or_else(|| fallback_family.clone());
        let style =
            find_name(&face, ttf_parser::name_id::SUBFAMILY).unwrap_or_else(|| "常规".to_string());
        faces.push(FontFace { family, style });
    }

    Ok(ParsedFont { format, faces })
}

fn find_name(face: &ttf_parser::Face<'_>, name_id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|name| name.name_id == name_id)
        .find_map(|name| name.to_string())
}

#[cfg(windows)]
pub fn register_font(path: &Path) -> Result<(), String> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::Graphics::Gdi::AddFontResourceExW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let added = unsafe { AddFontResourceExW(wide.as_ptr(), 0, ptr::null()) };
    if added > 0 {
        Ok(())
    } else {
        Err("AddFontResourceExW 返回 0".to_string())
    }
}

#[cfg(not(windows))]
pub fn register_font(_path: &Path) -> Result<(), String> {
    Err("FontLoader 仅支持 Windows".to_string())
}

#[cfg(windows)]
pub fn unregister_font(path: &Path) -> Result<(), String> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::Graphics::Gdi::RemoveFontResourceExW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let removed = unsafe { RemoveFontResourceExW(wide.as_ptr(), 0, ptr::null()) };
    if removed != 0 {
        Ok(())
    } else {
        Err("RemoveFontResourceExW 返回 0".to_string())
    }
}

#[cfg(not(windows))]
pub fn unregister_font(_path: &Path) -> Result<(), String> {
    Err("FontLoader 仅支持 Windows".to_string())
}

#[cfg(windows)]
pub fn broadcast_font_change() -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_FONTCHANGE,
    };

    let mut result = 0usize;
    let sent = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_FONTCHANGE,
            0,
            0,
            SMTO_ABORTIFHUNG,
            1_000,
            &mut result,
        )
    };
    if sent != 0 {
        Ok(())
    } else {
        Err("WM_FONTCHANGE 广播超时".to_string())
    }
}

#[cfg(not(windows))]
pub fn broadcast_font_change() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_extension, FontFormat};
    use std::path::Path;

    #[test]
    fn validates_extensions_case_insensitively() {
        assert_eq!(
            validate_extension(Path::new("font.TTF")).unwrap(),
            FontFormat::Ttf
        );
        assert_eq!(
            validate_extension(Path::new("font.otf")).unwrap(),
            FontFormat::Otf
        );
        assert_eq!(
            validate_extension(Path::new("font.TtC")).unwrap(),
            FontFormat::Ttc
        );
        assert!(validate_extension(Path::new("font.woff2")).is_err());
    }
}
