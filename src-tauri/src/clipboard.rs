use std::path::{Path, PathBuf};

use base64::Engine;
use sha2::{Digest, Sha256};

use crate::store::{content_hash_for, now_ms, Store, StoredItem, StoredPasteboard};
use crate::types::{primary_type, PasteType, Preview};

#[derive(Debug, Clone)]
pub struct CapturedItem {
    pub ty: PasteType,
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<Vec<u8>>,
    pub url: Option<String>,
    pub color: Option<String>,
    pub image: Option<Vec<u8>>,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CapturedPasteboard {
    pub items: Vec<CapturedItem>,
}

impl CapturedPasteboard {
    pub fn types(&self) -> Vec<PasteType> {
        self.items.iter().map(|i| i.ty).collect()
    }

    pub fn hash_parts(&self) -> Vec<(String, Vec<u8>)> {
        let mut parts = Vec::new();
        for item in &self.items {
            let key = item.ty.as_str().to_string();
            let bytes = if let Some(path) = &item.file_path {
                match std::fs::read(path) {
                    Ok(b) => Sha256::digest(&b).to_vec(),
                    Err(_) => path.to_string_lossy().as_bytes().to_vec(),
                }
            } else if let Some(img) = &item.image {
                Sha256::digest(img).to_vec()
            } else if let Some(rtf) = &item.rtf {
                rtf.clone()
            } else if let Some(html) = &item.html {
                html.as_bytes().to_vec()
            } else if let Some(url) = &item.url {
                url.as_bytes().to_vec()
            } else if let Some(color) = &item.color {
                color.as_bytes().to_vec()
            } else if let Some(text) = &item.text {
                text.as_bytes().to_vec()
            } else {
                Vec::new()
            };
            parts.push((key, bytes));
        }
        parts
    }

    pub fn content_hash(&self) -> String {
        content_hash_for(&self.hash_parts())
    }
}

/// Detect `#rgb` / `#rrggbb` from a whole-string copy.
pub fn parse_color(s: &str) -> Option<String> {
    let t = s.trim();
    if t.len() == 4 && t.starts_with('#') {
        let chars: Vec<char> = t.chars().collect();
        if chars[1..].iter().all(|c| c.is_ascii_hexdigit()) {
            return Some(t.to_ascii_lowercase());
        }
    }
    if t.len() == 7 && t.starts_with('#') {
        let chars: Vec<char> = t.chars().collect();
        if chars[1..].iter().all(|c| c.is_ascii_hexdigit()) {
            return Some(t.to_ascii_lowercase());
        }
    }
    None
}

pub fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((w, h))
}

fn truncate_title(s: &str, max: usize) -> String {
    let s = s.trim();
    let first = s.lines().next().unwrap_or(s);
    if first.chars().count() <= max {
        first.to_string()
    } else {
        let t: String = first.chars().take(max).collect();
        format!("{t}…")
    }
}

pub struct IngestResult {
    pub id: String,
    pub hash: String,
    pub total_bytes: u64,
}

pub fn ingest_captured(
    store: &mut Store,
    captured: &CapturedPasteboard,
    local_name: &str,
    local_id: &str,
) -> Result<Option<IngestResult>, String> {
    if captured.items.is_empty() {
        return Ok(None);
    }
    let hash = captured.content_hash();
    if store.last_content_hash()?.as_deref() == Some(hash.as_str()) {
        return Ok(None);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let ptype = primary_type(&captured.types());
    let mut preview = Preview::default();
    let mut items: Vec<StoredItem> = Vec::new();
    let mut total_bytes = 0u64;
    let mut title = String::from("剪贴板");

    for (idx, cap) in captured.items.iter().enumerate() {
        let item_id = uuid::Uuid::new_v4().to_string();
        let mut stored = StoredItem {
            id: item_id,
            pasteboard_id: id.clone(),
            sort_order: idx as i64,
            item_type: cap.ty.as_str().to_string(),
            text_content: cap.text.clone(),
            html_content: cap.html.clone(),
            rtf_b64: cap
                .rtf
                .as_ref()
                .map(|b| base64::engine::general_purpose::STANDARD.encode(b)),
            url: cap.url.clone(),
            color: cap.color.clone(),
            blob_hash: None,
            file_name: None,
            file_size: None,
            width: None,
            height: None,
            download_token: None,
        };

        match cap.ty {
            PasteType::File => {
                if let Some(path) = &cap.file_path {
                    let (hash, size) = store.ingest_file(path)?;
                    stored.blob_hash = Some(hash.clone());
                    stored.file_size = Some(size);
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "file".into());
                    stored.file_name = Some(name.clone());
                    stored.download_token = Some(hex::encode(rand_bytes(16)));
                    preview.file_name = Some(name.clone());
                    preview.file_size = Some(size);
                    preview.path = Some(store.blob_path(&hash).to_string_lossy().into_owned());
                    title = name;
                    total_bytes += size;
                }
            }
            PasteType::Image => {
                if let Some(bytes) = &cap.image {
                    let (hash, size) = store.put_blob(bytes)?;
                    stored.blob_hash = Some(hash.clone());
                    stored.file_size = Some(size);
                    if let Some((w, h)) = png_dimensions(bytes) {
                        stored.width = Some(w);
                        stored.height = Some(h);
                        preview.width = Some(w);
                        preview.height = Some(h);
                    }
                    if bytes.len() <= 80 * 1024 {
                        preview.image_thumb = Some(format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        ));
                    }
                    title = "图片".into();
                    total_bytes += size;
                }
            }
            PasteType::Html => {
                if let Some(html) = &cap.html {
                    preview.html = Some(truncate_title(html, 400));
                    title = truncate_title(html, 80);
                    total_bytes += html.len() as u64;
                }
            }
            PasteType::Rtf => {
                if let Some(rtf) = &cap.rtf {
                    title = "RTF".into();
                    total_bytes += rtf.len() as u64;
                }
            }
            PasteType::Url => {
                if let Some(url) = &cap.url {
                    preview.url = Some(url.clone());
                    title = truncate_title(url, 80);
                    total_bytes += url.len() as u64;
                }
            }
            PasteType::Color => {
                if let Some(c) = &cap.color {
                    preview.color = Some(c.clone());
                    title = c.clone();
                    total_bytes += c.len() as u64;
                }
            }
            PasteType::Text => {
                if let Some(t) = &cap.text {
                    preview.text = Some(truncate_title(t, 400));
                    title = truncate_title(t, 80);
                    total_bytes += t.len() as u64;
                }
            }
        }
        items.push(stored);
    }

    if preview.text.is_none() {
        if let Some(t) = captured.items.iter().find_map(|i| i.text.clone()) {
            preview.text = Some(truncate_title(&t, 400));
        }
    }

    let pb = StoredPasteboard {
        id: id.clone(),
        copied_at: now_ms(),
        source_device_id: Some(local_id.to_string()),
        source_device_name: local_name.to_string(),
        primary_type: ptype.as_str().to_string(),
        title,
        content_hash: hash.clone(),
        total_bytes,
        needs_file_download: false,
        file_download_state: "idle".into(),
        download_token: None,
        source_host: None,
        source_port: None,
        preview_json: serde_json::to_string(&preview).unwrap_or_else(|_| "{}".into()),
    };
    store.insert_pasteboard(&pb, &items)?;
    Ok(Some(IngestResult {
        id,
        hash,
        total_bytes,
    }))
}

fn rand_bytes(n: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

#[derive(Debug, Clone, Default)]
pub struct WritePayload {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<Vec<u8>>,
    pub url: Option<String>,
    pub image_png: Option<Vec<u8>>,
    pub file_paths: Vec<PathBuf>,
}

pub fn payload_from_store(store: &Store, pasteboard_id: &str) -> Result<WritePayload, String> {
    let items = store.get_items(pasteboard_id)?;
    let mut payload = WritePayload::default();
    for item in items {
        match item.item_type.as_str() {
            "text" | "color" => {
                if payload.text.is_none() {
                    payload.text = item.text_content.or(item.color);
                }
            }
            "html" => payload.html = item.html_content,
            "rtf" => {
                if let Some(b64) = item.rtf_b64 {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                        payload.rtf = Some(bytes);
                    }
                }
            }
            "url" => payload.url = item.url,
            "image" => {
                if let Some(hash) = item.blob_hash {
                    payload.image_png = Some(store.read_blob(&hash)?);
                }
            }
            "file" => {
                if let Some(hash) = item.blob_hash {
                    payload.file_paths.push(store.blob_path(&hash));
                }
            }
            _ => {}
        }
    }
    Ok(payload)
}

pub fn frontmost_app_name() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        macos::frontmost_app_name()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(String::new())
    }
}

pub fn pasteboard_change_count() -> Result<i64, String> {
    #[cfg(target_os = "macos")]
    {
        macos::change_count()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(0)
    }
}

pub fn read_native() -> Result<Option<CapturedPasteboard>, String> {
    #[cfg(target_os = "macos")]
    {
        macos::read()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(None)
    }
}

pub fn write_native(payload: &WritePayload) -> Result<i64, String> {
    #[cfg(target_os = "macos")]
    {
        macos::write(payload)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = payload;
        Ok(0)
    }
}

pub fn simulate_paste() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::simulate_cmd_v()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

pub fn activate_app_named(name: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::activate_app(name)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        Ok(())
    }
}

pub fn parse_file_url(s: &str) -> Option<PathBuf> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        let decoded = percent_encoding::percent_decode_str(rest)
            .decode_utf8()
            .ok()?;
        return Some(PathBuf::from(decoded.as_ref()));
    }
    if Path::new(s).exists() {
        return Some(PathBuf::from(s));
    }
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSApplicationActivationOptions, NSPasteboard, NSPasteboardTypeFileURL,
        NSPasteboardTypeHTML, NSPasteboardTypePNG, NSPasteboardTypeRTF, NSPasteboardTypeString,
        NSPasteboardTypeTIFF, NSPasteboardTypeURL, NSPasteboardWriting, NSWorkspace,
    };
    use objc2_foundation::{NSArray, NSData, NSString, NSURL};

    pub fn change_count() -> Result<i64, String> {
        let pb = NSPasteboard::generalPasteboard();
        Ok(pb.changeCount() as i64)
    }

    pub fn frontmost_app_name() -> Result<String, String> {
        let ws = NSWorkspace::sharedWorkspace();
        let app = ws.frontmostApplication();
        match app {
            Some(app) => {
                let name = app.localizedName();
                Ok(name.map(|s| s.to_string()).unwrap_or_default())
            }
            None => Ok(String::new()),
        }
    }

    pub fn activate_app(name: &str) -> Result<(), String> {
        let ws = NSWorkspace::sharedWorkspace();
        let apps = ws.runningApplications();
        for app in apps {
            let n = app.localizedName().map(|s| s.to_string()).unwrap_or_default();
            if n == name {
                #[allow(deprecated)]
                app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
                return Ok(());
            }
        }
        Ok(())
    }

    fn nsdata_bytes(data: &NSData) -> Vec<u8> {
        data.to_vec()
    }

    fn push_file(items: &mut Vec<CapturedItem>, path: PathBuf) {
        if path.exists() && items.iter().all(|i| i.file_path.as_ref() != Some(&path)) {
            items.push(CapturedItem {
                ty: PasteType::File,
                file_path: Some(path),
                ..empty_item()
            });
        }
    }

    pub fn read() -> Result<Option<CapturedPasteboard>, String> {
        let pb = NSPasteboard::generalPasteboard();
        let mut items: Vec<CapturedItem> = Vec::new();

        if let Some(s) = unsafe { pb.stringForType(NSPasteboardTypeFileURL) } {
            if let Some(path) = parse_file_url(&s.to_string()) {
                push_file(&mut items, path);
            }
        }
        if let Some(list) = unsafe { pb.propertyListForType(NSPasteboardTypeFileURL) } {
            let _ = list;
        }
        if let Some(items_arr) = pb.pasteboardItems() {
            for item in items_arr {
                if let Some(s) = unsafe { item.stringForType(NSPasteboardTypeFileURL) } {
                    if let Some(path) = parse_file_url(&s.to_string()) {
                        push_file(&mut items, path);
                    }
                }
            }
        }

        if let Some(html) = unsafe { pb.stringForType(NSPasteboardTypeHTML) } {
            let html = html.to_string();
            if !html.is_empty() {
                items.push(CapturedItem {
                    ty: PasteType::Html,
                    html: Some(html),
                    ..empty_item()
                });
            }
        }

        if let Some(data) = unsafe { pb.dataForType(NSPasteboardTypeRTF) } {
            let bytes = nsdata_bytes(&data);
            if !bytes.is_empty() {
                items.push(CapturedItem {
                    ty: PasteType::Rtf,
                    rtf: Some(bytes),
                    ..empty_item()
                });
            }
        }

        let png = unsafe { pb.dataForType(NSPasteboardTypePNG) };
        let tiff = unsafe { pb.dataForType(NSPasteboardTypeTIFF) };
        if let Some(data) = png.or(tiff) {
            let bytes = nsdata_bytes(&data);
            if !bytes.is_empty() {
                items.push(CapturedItem {
                    ty: PasteType::Image,
                    image: Some(bytes),
                    ..empty_item()
                });
            }
        }

        if items.iter().all(|i| i.ty != PasteType::Url) {
            if let Some(s) = unsafe { pb.stringForType(NSPasteboardTypeURL) } {
                let u = s.to_string();
                if !u.is_empty() && !u.starts_with("file:") {
                    items.push(CapturedItem {
                        ty: PasteType::Url,
                        url: Some(u),
                        ..empty_item()
                    });
                }
            }
        }

        if let Some(s) = unsafe { pb.stringForType(NSPasteboardTypeString) } {
            let text = s.to_string();
            if !text.is_empty() {
                if let Some(color) = parse_color(&text) {
                    items.push(CapturedItem {
                        ty: PasteType::Color,
                        color: Some(color.clone()),
                        text: Some(color),
                        ..empty_item()
                    });
                }
                items.push(CapturedItem {
                    ty: PasteType::Text,
                    text: Some(text),
                    ..empty_item()
                });
            }
        }

        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(CapturedPasteboard { items }))
    }

    fn empty_item() -> CapturedItem {
        CapturedItem {
            ty: PasteType::Text,
            text: None,
            html: None,
            rtf: None,
            url: None,
            color: None,
            image: None,
            file_path: None,
        }
    }

    pub fn write(payload: &WritePayload) -> Result<i64, String> {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();

        if !payload.file_paths.is_empty() {
            let mut writers = Vec::new();
            for path in &payload.file_paths {
                let s = NSString::from_str(&path.to_string_lossy());
                let url = NSURL::fileURLWithPath_isDirectory(&s, path.is_dir());
                writers.push(ProtocolObject::<dyn NSPasteboardWriting>::from_retained(url));
            }
            let array = NSArray::from_retained_slice(&writers);
            let _ = pb.writeObjects(&array);
        }

        if let Some(png) = &payload.image_png {
            let data = NSData::with_bytes(png);
            let _ = unsafe { pb.setData_forType(Some(&data), NSPasteboardTypePNG) };
        }
        if let Some(html) = &payload.html {
            let s = NSString::from_str(html);
            let _ = unsafe { pb.setString_forType(&s, NSPasteboardTypeHTML) };
        }
        if let Some(rtf) = &payload.rtf {
            let data = NSData::with_bytes(rtf);
            let _ = unsafe { pb.setData_forType(Some(&data), NSPasteboardTypeRTF) };
        }
        if let Some(url) = &payload.url {
            let s = NSString::from_str(url);
            let _ = unsafe { pb.setString_forType(&s, NSPasteboardTypeURL) };
            if payload.text.is_none() {
                let _ = unsafe { pb.setString_forType(&s, NSPasteboardTypeString) };
            }
        }
        if let Some(text) = &payload.text {
            let s = NSString::from_str(text);
            let _ = unsafe { pb.setString_forType(&s, NSPasteboardTypeString) };
        }

        Ok(pb.changeCount() as i64)
    }

    pub fn simulate_cmd_v() -> Result<(), String> {
        use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        const KEY_V: u16 = 9;
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "无法创建按键事件".to_string())?;
        let down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
            .map_err(|_| "无法创建按键按下".to_string())?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        down.post(CGEventTapLocation::HID);
        let up = CGEvent::new_keyboard_event(source, KEY_V, false)
            .map_err(|_| "无法创建按键抬起".to_string())?;
        up.set_flags(CGEventFlags::CGEventFlagCommand);
        up.post(CGEventTapLocation::HID);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_from_text() {
        assert_eq!(parse_color("#abc"), Some("#abc".into()));
        assert_eq!(parse_color("#AABBCC"), Some("#aabbcc".into()));
        assert_eq!(parse_color("  #ff00aa  "), Some("#ff00aa".into()));
        assert_eq!(parse_color("#ggg"), None);
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#ffff"), None);
    }

    #[test]
    fn primary_type_from_captured() {
        let cap = CapturedPasteboard {
            items: vec![
                CapturedItem {
                    ty: PasteType::Text,
                    text: Some("hi".into()),
                    html: None,
                    rtf: None,
                    url: None,
                    color: None,
                    image: None,
                    file_path: None,
                },
                CapturedItem {
                    ty: PasteType::Html,
                    html: Some("<b>hi</b>".into()),
                    text: None,
                    rtf: None,
                    url: None,
                    color: None,
                    image: None,
                    file_path: None,
                },
            ],
        };
        assert_eq!(primary_type(&cap.types()), PasteType::Html);
    }

    #[test]
    fn dedup_hash_stable() {
        let a = CapturedPasteboard {
            items: vec![CapturedItem {
                ty: PasteType::Text,
                text: Some("same".into()),
                html: None,
                rtf: None,
                url: None,
                color: None,
                image: None,
                file_path: None,
            }],
        };
        let b = a.clone();
        assert_eq!(a.content_hash(), b.content_hash());
    }
}
