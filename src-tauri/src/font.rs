use crate::models::{FontFace, FontFormat};
use std::{fs, path::Path};
use ttf_parser::PlatformId;

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
        let mut family_names: Vec<_> = face
            .names()
            .into_iter()
            .filter(|name| name.name_id == ttf_parser::name_id::FAMILY)
            .filter_map(|name| {
                let priority = match (name.platform_id, name.language_id, name.encoding_id) {
                    (PlatformId::Windows, 0x0409, _) => 0,
                    (PlatformId::Windows, _, _)
                        if name.language().primary_language() == "English" =>
                    {
                        1
                    }
                    (PlatformId::Macintosh, 0, 0) => 2,
                    _ => 3,
                };
                let value = match (name.platform_id, name.encoding_id) {
                    (PlatformId::Windows, 10) if name.name.len() % 2 == 0 => {
                        let units: Vec<_> = name
                            .name
                            .chunks_exact(2)
                            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
                            .collect();
                        String::from_utf16(&units).ok()
                    }
                    #[cfg(windows)]
                    (PlatformId::Macintosh, 0) if name.language_id == 0 => {
                        decode_mac_roman(name.name)
                    }
                    _ => name.to_string(),
                }?;
                (!value.trim().is_empty()).then_some((priority, value))
            })
            .collect();
        family_names.sort_by_key(|(priority, _)| *priority);
        let mut family_names = family_names.into_iter().map(|(_, name)| name);
        let family = family_names
            .next()
            .unwrap_or_else(|| fallback_family.clone());
        let mut family_aliases = Vec::new();
        for name in family_names {
            if name != family && !family_aliases.contains(&name) {
                family_aliases.push(name);
            }
        }
        let style =
            find_name(&face, ttf_parser::name_id::SUBFAMILY).unwrap_or_else(|| "常规".to_string());
        faces.push(FontFace {
            family,
            family_aliases,
            style,
        });
    }

    Ok(ParsedFont { format, faces })
}

#[cfg(windows)]
fn decode_mac_roman(bytes: &[u8]) -> Option<String> {
    use windows_sys::Win32::Globalization::MultiByteToWideChar;

    const MAC_ROMAN_CODE_PAGE: u32 = 10000;
    let mut decoded = vec![0; bytes.len()];
    let written = unsafe {
        MultiByteToWideChar(
            MAC_ROMAN_CODE_PAGE,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            decoded.as_mut_ptr(),
            decoded.len() as i32,
        )
    };
    if written == 0 {
        return None;
    }
    String::from_utf16(&decoded[..written as usize]).ok()
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
        SendNotifyMessageW, HWND_BROADCAST, WM_FONTCHANGE,
    };

    let sent = unsafe { SendNotifyMessageW(HWND_BROADCAST, WM_FONTCHANGE, 0, 0) };
    if sent != 0 {
        Ok(())
    } else {
        Err(format!(
            "WM_FONTCHANGE 广播失败：{}",
            std::io::Error::last_os_error()
        ))
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
