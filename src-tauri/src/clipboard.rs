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
                    preview.path = Some(
                        store
                            .named_blob_path(&hash, Some(&name))
                            .unwrap_or_else(|_| store.blob_path(&hash))
                            .to_string_lossy()
                            .into_owned(),
                    );
                    if ptype == PasteType::File {
                        title = name;
                    }
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
                    if ptype == PasteType::Image {
                        title = "图片".into();
                    }
                    total_bytes += size;
                }
            }
            PasteType::Html => {
                if let Some(html) = &cap.html {
                    preview.html = Some(truncate_title(html, 400));
                    if ptype == PasteType::Html {
                        title = truncate_title(html, 80);
                    }
                    total_bytes += html.len() as u64;
                }
            }
            PasteType::Rtf => {
                if let Some(rtf) = &cap.rtf {
                    if ptype == PasteType::Rtf {
                        title = "RTF".into();
                    }
                    total_bytes += rtf.len() as u64;
                }
            }
            PasteType::Url => {
                if let Some(url) = &cap.url {
                    preview.url = Some(url.clone());
                    if ptype == PasteType::Url {
                        title = truncate_title(url, 80);
                    }
                    total_bytes += url.len() as u64;
                }
            }
            PasteType::Color => {
                if let Some(c) = &cap.color {
                    preview.color = Some(c.clone());
                    if ptype == PasteType::Color {
                        title = c.clone();
                    }
                    total_bytes += c.len() as u64;
                }
            }
            PasteType::Text => {
                if let Some(t) = &cap.text {
                    if ptype != PasteType::File {
                        preview.text = Some(truncate_title(t, 400));
                    }
                    if ptype == PasteType::Text {
                        title = truncate_title(t, 80);
                    }
                    total_bytes += t.len() as u64;
                }
            }
        }
        items.push(stored);
    }

    if preview.text.is_none() && ptype != PasteType::File && ptype != PasteType::Image {
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
                    let path = store
                        .named_blob_path(&hash, item.file_name.as_deref())
                        .unwrap_or_else(|_| store.blob_path(&hash));
                    payload.file_paths.push(path);
                }
            }
            _ => {}
        }
    }
    if !payload.file_paths.is_empty() {
        payload.text = None;
        payload.html = None;
        payload.rtf = None;
        payload.url = None;
        payload.image_png = None;
    }
    Ok(payload)
}

pub fn remember_frontmost() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        win32::remember_frontmost()
    }
    #[cfg(not(target_os = "windows"))]
    {
        frontmost_app_name()
    }
}

pub fn track_frontmost() {
    #[cfg(target_os = "windows")]
    {
        win32::track_frontmost();
    }
}

pub fn frontmost_app_name() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        macos::frontmost_app_name()
    }
    #[cfg(target_os = "windows")]
    {
        win32::frontmost_app_name()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(String::new())
    }
}

pub fn pasteboard_change_count() -> Result<i64, String> {
    #[cfg(target_os = "macos")]
    {
        macos::change_count()
    }
    #[cfg(target_os = "windows")]
    {
        win32::change_count()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(0)
    }
}

pub fn read_native() -> Result<Option<CapturedPasteboard>, String> {
    #[cfg(target_os = "macos")]
    {
        macos::read()
    }
    #[cfg(target_os = "windows")]
    {
        win32::read()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(None)
    }
}

pub fn write_native(payload: &WritePayload) -> Result<i64, String> {
    #[cfg(target_os = "macos")]
    {
        macos::write(payload)
    }
    #[cfg(target_os = "windows")]
    {
        win32::write(payload)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
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
    #[cfg(target_os = "windows")]
    {
        win32::simulate_ctrl_v()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(())
    }
}

pub fn activate_app_named(name: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::activate_app(name)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = name;
        win32::activate_remembered()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = name;
        Ok(())
    }
}

fn pasteboard_has_file(items: &[CapturedItem]) -> bool {
    items.iter().any(|i| i.ty == PasteType::File)
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

        if !pasteboard_has_file(&items) {
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

#[cfg(target_os = "windows")]
mod win32 {
    use super::*;
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::Duration;

    use windows::core::{w, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentProcessId, OpenProcess,
        QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_V,
    };
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, GetAncestor, GetClassNameW, GetForegroundWindow,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
        SetForegroundWindow, ShowWindow, SwitchToThisWindow, ASFW_ANY, GA_ROOT, SW_RESTORE,
    };

    struct PrevTarget {
        hwnd: isize,
        name: String,
    }

    static PREV: Mutex<PrevTarget> = Mutex::new(PrevTarget {
        hwnd: 0,
        name: String::new(),
    });

    fn prev_lock() -> std::sync::MutexGuard<'static, PrevTarget> {
        PREV.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn hwnd_from_isize(v: isize) -> HWND {
        HWND(v as *mut c_void)
    }

    fn hwnd_as_isize(hwnd: HWND) -> isize {
        hwnd.0 as isize
    }

    fn hwnd_null(hwnd: HWND) -> bool {
        hwnd.0.is_null()
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

    fn window_title(hwnd: HWND) -> String {
        unsafe {
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(hwnd, &mut buf);
            if n <= 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }

    fn is_own_process(hwnd: HWND) -> bool {
        unsafe {
            if hwnd_null(hwnd) {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
            pid != 0 && pid == GetCurrentProcessId()
        }
    }

    fn process_name(hwnd: HWND) -> String {
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
            if pid != 0 {
                if let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                    let mut buf = [0u16; 512];
                    let mut size = buf.len() as u32;
                    let ok = QueryFullProcessImageNameW(
                        proc,
                        PROCESS_NAME_WIN32,
                        PWSTR(buf.as_mut_ptr()),
                        &mut size,
                    )
                    .is_ok();
                    let _ = CloseHandle(proc);
                    if ok && size > 0 {
                        let path = String::from_utf16_lossy(&buf[..size as usize]);
                        if let Some(stem) = Path::new(&path).file_stem() {
                            let name = stem.to_string_lossy().into_owned();
                            if !name.is_empty() {
                                return name;
                            }
                        }
                    }
                }
            }
            window_title(hwnd)
        }
    }

    fn class_name(hwnd: HWND) -> String {
        unsafe {
            let mut buf = [0u16; 256];
            let n = GetClassNameW(hwnd, &mut buf);
            if n <= 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }

    fn top_level(hwnd: HWND) -> HWND {
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        if hwnd_null(root) {
            hwnd
        } else {
            root
        }
    }

    fn is_shell_window(hwnd: HWND) -> bool {
        let class = class_name(hwnd);
        matches!(
            class.as_str(),
            "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "NotifyIconOverflowWindow"
                | "Progman"
                | "WorkerW"
                | "ForegroundStaging"
                | "Windows.Internal.Shell.TabProxyWindow"
                | "#32769"
        )
    }

    fn is_usable_target(hwnd: HWND) -> bool {
        unsafe {
            if hwnd_null(hwnd) || !IsWindow(Some(hwnd)).as_bool() {
                return false;
            }
            if !IsWindowVisible(hwnd).as_bool() {
                return false;
            }
            if is_own_process(hwnd) || is_shell_window(hwnd) {
                return false;
            }
            true
        }
    }

    pub fn track_frontmost() {
        let hwnd = top_level(unsafe { GetForegroundWindow() });
        if !is_usable_target(hwnd) {
            return;
        }
        let name = process_name(hwnd);
        let mut g = prev_lock();
        g.hwnd = hwnd_as_isize(hwnd);
        g.name = name;
    }

    pub fn remember_frontmost() -> Result<String, String> {
        track_frontmost();
        Ok(prev_lock().name.clone())
    }

    pub fn frontmost_app_name() -> Result<String, String> {
        let hwnd = unsafe { GetForegroundWindow() };
        if is_own_process(hwnd) {
            return Ok(prev_lock().name.clone());
        }
        Ok(process_name(hwnd))
    }

    pub fn change_count() -> Result<i64, String> {
        Ok(unsafe { GetClipboardSequenceNumber() as i64 })
    }

    fn force_foreground(hwnd: HWND) {
        unsafe {
            if hwnd_null(hwnd) || !IsWindow(Some(hwnd)).as_bool() {
                return;
            }
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            let fg = GetForegroundWindow();
            if !hwnd_null(fg) && hwnd_as_isize(fg) == hwnd_as_isize(hwnd) {
                return;
            }

            // Attach the *current foreground* thread to the target, not this worker.
            let fg_tid = if hwnd_null(fg) {
                0
            } else {
                GetWindowThreadProcessId(fg, None)
            };
            let target_tid = GetWindowThreadProcessId(hwnd, None);
            let attached = fg_tid != 0
                && target_tid != 0
                && fg_tid != target_tid
                && AttachThreadInput(fg_tid, target_tid, true).as_bool();

            let _ = AllowSetForegroundWindow(ASFW_ANY);
            SwitchToThisWindow(hwnd, true);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);

            if attached {
                let _ = AttachThreadInput(fg_tid, target_tid, false);
            }
        }
    }

    pub fn activate_remembered() -> Result<(), String> {
        let hwnd_val = prev_lock().hwnd;
        if hwnd_val == 0 {
            return Ok(());
        }
        force_foreground(hwnd_from_isize(hwnd_val));
        Ok(())
    }

    fn key_input(vk: VIRTUAL_KEY, scan: u16, up: bool) -> INPUT {
        let flags = if up {
            KEYEVENTF_KEYUP
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
        let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err("无法发送按键".into());
        }
        Ok(())
    }

    pub fn simulate_ctrl_v() -> Result<(), String> {
        // VK_CONTROL + left-Ctrl scan (0x1D). VK_LCONTROL alone often arrives as a
        // plain "v" because ToUnicode/GetKeyState look at VK_CONTROL.
        const SCAN_LCTRL: u16 = 0x1D;
        let scan_v = unsafe { MapVirtualKeyW(u32::from(VK_V.0), MAPVK_VK_TO_VSC) } as u16;
        send_inputs(&[key_input(VK_CONTROL, SCAN_LCTRL, false)])?;
        std::thread::sleep(Duration::from_millis(15));
        send_inputs(&[
            key_input(VK_V, scan_v, false),
            key_input(VK_V, scan_v, true),
        ])?;
        std::thread::sleep(Duration::from_millis(10));
        send_inputs(&[key_input(VK_CONTROL, SCAN_LCTRL, true)])
    }

    fn with_clipboard<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        unsafe {
            let mut opened = false;
            for _ in 0..8 {
                if OpenClipboard(None).is_ok() {
                    opened = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if !opened {
                return Err("无法打开剪贴板".into());
            }
            let result = f();
            let _ = CloseClipboard();
            result
        }
    }

    unsafe fn alloc_bytes(bytes: &[u8]) -> Result<HANDLE, String> {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| e.to_string())?;
        let ptr = GlobalLock(h);
        if ptr.is_null() {
            let _ = GlobalFree(Some(h));
            return Err("GlobalLock 失败".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
        let _ = GlobalUnlock(h);
        Ok(HANDLE(h.0))
    }

    unsafe fn set_clipboard_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
        let handle = alloc_bytes(bytes)?;
        match SetClipboardData(format, Some(handle)) {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = GlobalFree(Some(HGLOBAL(handle.0)));
                Err(e.to_string())
            }
        }
    }

    fn utf16_bytes_nul(s: &str) -> Vec<u8> {
        let mut wide: Vec<u16> = s.encode_utf16().collect();
        wide.push(0);
        let mut bytes = Vec::with_capacity(wide.len() * 2);
        for u in wide {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        bytes
    }

    fn html_clipboard_payload(html: &str) -> Vec<u8> {
        let start_frag = "<!--StartFragment-->";
        let end_frag = "<!--EndFragment-->";
        let body = format!("<html>\r\n<body>\r\n{start_frag}{html}{end_frag}\r\n</body>\r\n</html>");
        let dummy = format!(
            "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
            0, 0, 0, 0
        );
        let start_html = dummy.len();
        let start_fragment = start_html + body.find(start_frag).unwrap_or(0) + start_frag.len();
        let end_fragment = start_html + body.find(end_frag).unwrap_or(body.len());
        let end_html = start_html + body.len();
        let header = format!(
            "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
            start_html, end_html, start_fragment, end_fragment
        );
        let mut out = header.into_bytes();
        out.extend_from_slice(body.as_bytes());
        out.push(0);
        out
    }

    fn hdrop_bytes(paths: &[PathBuf]) -> Vec<u8> {
        use std::os::windows::ffi::OsStrExt;
        let mut files: Vec<u16> = Vec::new();
        for p in paths {
            files.extend(p.as_os_str().encode_wide());
            files.push(0);
        }
        files.push(0);
        let header_size = 20u32; // DROPFILES: u32 + POINT(8) + BOOL + BOOL
        let mut bytes = vec![0u8; header_size as usize + files.len() * 2];
        bytes[0..4].copy_from_slice(&header_size.to_le_bytes());
        // fWide = TRUE at offset 16
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        let mut off = header_size as usize;
        for u in files {
            bytes[off..off + 2].copy_from_slice(&u.to_le_bytes());
            off += 2;
        }
        bytes
    }

    pub fn write(payload: &WritePayload) -> Result<i64, String> {
        with_clipboard(|| unsafe {
            EmptyClipboard().map_err(|e| e.to_string())?;
            if !payload.file_paths.is_empty() {
                set_clipboard_bytes(u32::from(CF_HDROP.0), &hdrop_bytes(&payload.file_paths))?;
            }
            if let Some(png) = &payload.image_png {
                let fmt = RegisterClipboardFormatW(w!("PNG"));
                if fmt != 0 {
                    set_clipboard_bytes(fmt, png)?;
                }
            }
            if let Some(html) = &payload.html {
                let fmt = RegisterClipboardFormatW(w!("HTML Format"));
                if fmt != 0 {
                    set_clipboard_bytes(fmt, &html_clipboard_payload(html))?;
                }
            }
            if let Some(rtf) = &payload.rtf {
                let fmt = RegisterClipboardFormatW(w!("Rich Text Format"));
                if fmt != 0 {
                    set_clipboard_bytes(fmt, rtf)?;
                }
            }
            let text = payload
                .text
                .clone()
                .or_else(|| payload.url.clone());
            if let Some(text) = text {
                set_clipboard_bytes(u32::from(CF_UNICODETEXT.0), &utf16_bytes_nul(&text))?;
            }
            Ok(GetClipboardSequenceNumber() as i64)
        })
    }

    unsafe fn handle_bytes(handle: HANDLE) -> Option<Vec<u8>> {
        if handle.0.is_null() {
            return None;
        }
        let hg = HGLOBAL(handle.0);
        let size = GlobalSize(hg);
        if size == 0 {
            return None;
        }
        let ptr = GlobalLock(hg);
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
        let _ = GlobalUnlock(hg);
        Some(bytes)
    }

    fn bytes_to_utf16_string(bytes: &[u8]) -> String {
        let n = bytes.len() / 2;
        let mut wide = Vec::with_capacity(n);
        let mut i = 0;
        while i + 1 < bytes.len() {
            let u = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
            if u == 0 {
                break;
            }
            wide.push(u);
            i += 2;
        }
        String::from_utf16_lossy(&wide)
    }

    fn parse_html_format(bytes: &[u8]) -> Option<String> {
        let s = std::str::from_utf8(bytes).ok().or_else(|| {
            let c0 = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            std::str::from_utf8(&bytes[..c0]).ok()
        })?;
        if let (Some(a), Some(b)) = (s.find("<!--StartFragment-->"), s.find("<!--EndFragment-->"))
        {
            let start = a + "<!--StartFragment-->".len();
            if start <= b {
                return Some(s[start..b].to_string());
            }
        }
        s.find("<html")
            .or_else(|| s.find("<HTML"))
            .map(|i| s[i..].trim_end_matches('\0').to_string())
    }

    fn looks_like_url(s: &str) -> bool {
        let t = s.trim();
        t.starts_with("http://") || t.starts_with("https://")
    }

    fn read_hdrop() -> Vec<PathBuf> {
        unsafe {
            let Ok(handle) = GetClipboardData(u32::from(CF_HDROP.0)) else {
                return Vec::new();
            };
            if handle.0.is_null() {
                return Vec::new();
            }
            let hdrop = HDROP(handle.0);
            let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
            let mut paths = Vec::new();
            for i in 0..count {
                let mut needed = DragQueryFileW(hdrop, i, None) as usize;
                if needed == 0 {
                    continue;
                }
                needed += 1;
                let mut buf = vec![0u16; needed];
                let n = DragQueryFileW(hdrop, i, Some(&mut buf));
                if n > 0 {
                    let path = String::from_utf16_lossy(&buf[..n as usize]);
                    if !path.is_empty() {
                        paths.push(PathBuf::from(path));
                    }
                }
            }
            paths
        }
    }

    pub fn read() -> Result<Option<CapturedPasteboard>, String> {
        with_clipboard(|| {
            let mut items: Vec<CapturedItem> = Vec::new();
            unsafe {
                if IsClipboardFormatAvailable(u32::from(CF_HDROP.0)).is_ok() {
                    for path in read_hdrop() {
                        if path.exists() {
                            items.push(CapturedItem {
                                ty: PasteType::File,
                                file_path: Some(path),
                                ..empty_item()
                            });
                        }
                    }
                }

                if !pasteboard_has_file(&items) {
                    let png_fmt = RegisterClipboardFormatW(w!("PNG"));
                    if png_fmt != 0 && IsClipboardFormatAvailable(png_fmt).is_ok() {
                        if let Ok(handle) = GetClipboardData(png_fmt) {
                            if let Some(bytes) = handle_bytes(handle) {
                                if !bytes.is_empty() {
                                    items.push(CapturedItem {
                                        ty: PasteType::Image,
                                        image: Some(bytes),
                                        ..empty_item()
                                    });
                                }
                            }
                        }
                    }
                }

                let html_fmt = RegisterClipboardFormatW(w!("HTML Format"));
                if html_fmt != 0 && IsClipboardFormatAvailable(html_fmt).is_ok() {
                    if let Ok(handle) = GetClipboardData(html_fmt) {
                        if let Some(bytes) = handle_bytes(handle) {
                            if let Some(html) = parse_html_format(&bytes) {
                                if !html.is_empty() {
                                    items.push(CapturedItem {
                                        ty: PasteType::Html,
                                        html: Some(html),
                                        ..empty_item()
                                    });
                                }
                            }
                        }
                    }
                }

                let rtf_fmt = RegisterClipboardFormatW(w!("Rich Text Format"));
                if rtf_fmt != 0 && IsClipboardFormatAvailable(rtf_fmt).is_ok() {
                    if let Ok(handle) = GetClipboardData(rtf_fmt) {
                        if let Some(bytes) = handle_bytes(handle) {
                            if !bytes.is_empty() {
                                items.push(CapturedItem {
                                    ty: PasteType::Rtf,
                                    rtf: Some(bytes),
                                    ..empty_item()
                                });
                            }
                        }
                    }
                }

                if IsClipboardFormatAvailable(u32::from(CF_UNICODETEXT.0)).is_ok() {
                    if let Ok(handle) = GetClipboardData(u32::from(CF_UNICODETEXT.0)) {
                        if let Some(bytes) = handle_bytes(handle) {
                            let text = bytes_to_utf16_string(&bytes);
                            if !text.is_empty() {
                                if looks_like_url(&text)
                                    && items.iter().all(|i| i.ty != PasteType::Url)
                                {
                                    items.push(CapturedItem {
                                        ty: PasteType::Url,
                                        url: Some(text.trim().to_string()),
                                        ..empty_item()
                                    });
                                }
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
                    }
                }
            }
            if items.is_empty() {
                Ok(None)
            } else {
                Ok(Some(CapturedPasteboard { items }))
            }
        })
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

    #[test]
    fn file_pasteboard_skips_image_sidecar() {
        let items = vec![CapturedItem {
            ty: PasteType::File,
            text: None,
            html: None,
            rtf: None,
            url: None,
            color: None,
            image: None,
            file_path: Some(PathBuf::from("/tmp/a.bin")),
        }];
        assert!(pasteboard_has_file(&items));
        let empty: Vec<CapturedItem> = Vec::new();
        assert!(!pasteboard_has_file(&empty));
    }

    fn cap_file(path: PathBuf) -> CapturedItem {
        CapturedItem {
            ty: PasteType::File,
            text: None,
            html: None,
            rtf: None,
            url: None,
            color: None,
            image: None,
            file_path: Some(path),
        }
    }

    fn cap_text(text: &str) -> CapturedItem {
        CapturedItem {
            ty: PasteType::Text,
            text: Some(text.into()),
            html: None,
            rtf: None,
            url: None,
            color: None,
            image: None,
            file_path: None,
        }
    }

    #[test]
    fn file_ingest_keeps_name_despite_text_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let src = dir.path().join("新增 文本文档.txt");
        std::fs::write(&src, b"hello").unwrap();
        let captured = CapturedPasteboard {
            items: vec![
                cap_file(src),
                cap_text(r"C:\Users\me\Desktop\新增 文本文档.txt"),
            ],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev")
            .unwrap()
            .unwrap();
        let pb = store.get_pasteboard(&result.id).unwrap().unwrap();
        assert_eq!(pb.primary_type, "file");
        assert_eq!(pb.title, "新增 文本文档.txt");
        let preview: crate::types::Preview =
            serde_json::from_str(&pb.preview_json).unwrap();
        assert_eq!(preview.file_name.as_deref(), Some("新增 文本文档.txt"));
        assert!(preview.text.is_none());
        let payload = payload_from_store(&store, &result.id).unwrap();
        assert_eq!(
            payload.file_paths[0].file_name().unwrap().to_string_lossy(),
            "新增 文本文档.txt"
        );
        assert!(payload.text.is_none());
    }
}
