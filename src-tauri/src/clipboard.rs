use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use image::{DynamicImage, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};

use crate::store::{content_hash_for, now_ms, Store, StoredItem, StoredPasteboard};
use crate::types::{primary_type, PasteType, Preview};

pub type FileFulfill = Arc<dyn Fn(String) -> Result<PathBuf, String> + Send + Sync>;

static OWN_PROMISE_COUNT: AtomicI64 = AtomicI64::new(-1);
static SKIP_CLIPBOARD_READ: AtomicBool = AtomicBool::new(false);
/// Ignore clipboard change-count ticks caused by our own writes. Extra
/// NSPasteboard / Win32 updates often arrive after `write` returns, so a single
/// `suppress_change_count` match is not enough and the watcher would ingest the
/// same payload again and auto-sync it back to the sender.
static OWN_WRITE_UNTIL_MS: AtomicI64 = AtomicI64::new(0);
const OWN_WRITE_GRACE_MS: i64 = 800;
static FILE_FULFILL: Mutex<Option<FileFulfill>> = Mutex::new(None);
static FILE_PROMISE_ID: Mutex<Option<String>> = Mutex::new(None);

pub fn install_file_fulfill(f: FileFulfill) {
    if let Ok(mut g) = FILE_FULFILL.lock() {
        *g = Some(f);
    }
}

pub fn note_own_promise(count: Option<i64>) {
    OWN_PROMISE_COUNT.store(count.unwrap_or(-1), Ordering::SeqCst);
    if count.is_none() {
        if let Ok(mut g) = FILE_PROMISE_ID.lock() {
            *g = None;
        }
    }
}

pub fn mark_own_write(count: i64) {
    note_own_promise(Some(count));
    let until = crate::store::now_ms().saturating_add(OWN_WRITE_GRACE_MS);
    OWN_WRITE_UNTIL_MS.store(until, Ordering::SeqCst);
}

pub fn own_write_in_grace() -> bool {
    crate::store::now_ms() < OWN_WRITE_UNTIL_MS.load(Ordering::SeqCst)
}

pub fn own_promise_active() -> bool {
    if SKIP_CLIPBOARD_READ.load(Ordering::SeqCst) {
        return true;
    }
    let marked = OWN_PROMISE_COUNT.load(Ordering::SeqCst);
    if marked < 0 {
        return false;
    }
    pasteboard_change_count()
        .map(|c| c == marked)
        .unwrap_or(false)
}

pub fn should_ignore_own_change(count: i64, suppress: i64) -> bool {
    count == suppress || own_promise_active() || own_write_in_grace()
}

#[cfg(target_os = "windows")]
fn current_promise_id() -> Option<String> {
    FILE_PROMISE_ID.lock().ok().and_then(|g| g.clone())
}

#[cfg(target_os = "windows")]
fn fulfill_promised_file() -> Result<PathBuf, String> {
    let id = current_promise_id().ok_or_else(|| "没有待获取的文件".to_string())?;
    fulfill_promised_file_id(&id)
}

fn fulfill_promised_file_id(id: &str) -> Result<PathBuf, String> {
    let f = FILE_FULFILL
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| "文件获取未就绪".to_string())?;
    f(id.to_string())
}

/// Place a delayed file on the system clipboard. Bytes are fetched when another
/// app pastes (NSPasteboardItemDataProvider / CF_HDROP delayed rendering).
pub fn write_file_promise(pasteboard_id: &str) -> Result<i64, String> {
    SKIP_CLIPBOARD_READ.store(true, Ordering::SeqCst);
    if let Ok(mut g) = FILE_PROMISE_ID.lock() {
        *g = Some(pasteboard_id.to_string());
    }
    let result = {
        #[cfg(target_os = "macos")]
        {
            macos::write_promise(pasteboard_id)
        }
        #[cfg(target_os = "windows")]
        {
            let _ = pasteboard_id;
            win32::write_delayed_hdrop()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = pasteboard_id;
            Err("此平台不支持延迟文件剪贴板".into())
        }
    };
    match &result {
        Ok(count) => mark_own_write(*count),
        Err(_) => note_own_promise(None),
    }
    SKIP_CLIPBOARD_READ.store(false, Ordering::SeqCst);
    result
}

#[cfg(target_os = "windows")]
pub fn install_clipboard_owner_hwnd(hwnd: isize) {
    win32::subclass_owner(hwnd);
}

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

fn normalize_visible(s: &str) -> String {
    s.replace('\u{00a0}', " ")
        .replace('\u{200b}', "")
        .replace('\u{feff}', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn entity_char(ent: &str) -> Option<char> {
    match ent {
        "nbsp" => Some('\u{00a0}'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        _ if ent.starts_with('#') => {
            let num = if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                ent[1..].parse().ok()?
            };
            char::from_u32(num)
        }
        _ => None,
    }
}

fn decode_entities_push(s: &str, out: &mut String) {
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(j) = rest.find(';') else {
            out.push_str(rest);
            return;
        };
        if j > 32 {
            out.push('&');
            rest = &rest[1..];
            continue;
        }
        match entity_char(&rest[1..j]) {
            Some(ch) => out.push(ch),
            None => out.push_str(&rest[..=j]),
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
}

fn is_block_or_break(tag: &str) -> bool {
    matches!(
        tag,
        "br" | "p"
            | "div"
            | "tr"
            | "li"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "pre"
            | "hr"
            | "dt"
            | "dd"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "nav"
            | "figure"
            | "figcaption"
    )
}

fn tag_name_of(tag: &str) -> String {
    tag.trim()
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn skip_until_close<'a>(rest: &'a str, name: &str) -> &'a str {
    let closer = format!("</{name}>");
    let lower = rest.to_ascii_lowercase();
    match lower.find(&closer) {
        Some(i) => &rest[i + closer.len()..],
        None => "",
    }
}

/// Visible text from clipboard HTML, skipping head/script/style and comments.
pub fn html_visible_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(i) = rest.find('<') else {
            decode_entities_push(rest, &mut out);
            break;
        };
        decode_entities_push(&rest[..i], &mut out);
        rest = &rest[i..];
        if rest.starts_with("<!--") {
            rest = match rest[4..].find("-->") {
                Some(j) => &rest[4 + j + 3..],
                None => "",
            };
            continue;
        }
        let Some(end) = rest.find('>') else {
            break;
        };
        let name = tag_name_of(&rest[1..end]);
        rest = &rest[end + 1..];
        if name == "script" || name == "style" || name == "head" || name == "noscript" {
            rest = skip_until_close(rest, &name);
            continue;
        }
        if is_block_or_break(&name) {
            out.push('\n');
        }
    }
    out
}

fn has_open_tag(lower_html: &str, name: &str) -> bool {
    let needle = format!("<{name}");
    let mut from = 0;
    while let Some(i) = lower_html[from..].find(&needle) {
        let after = from + i + needle.len();
        let next = lower_html[after..].chars().next();
        if next
            .map(|c| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(true)
        {
            return true;
        }
        from = after;
    }
    false
}

fn open_tag_has_attr(lower_html: &str, name: &str, attr: &str) -> bool {
    let needle = format!("<{name}");
    let mut from = 0;
    while let Some(i) = lower_html[from..].find(&needle) {
        let after = from + i + needle.len();
        let next = lower_html[after..].chars().next();
        if !next
            .map(|c| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(true)
        {
            from = after;
            continue;
        }
        let rest = &lower_html[after..];
        let end = rest.find('>').unwrap_or(rest.len());
        let attrs = &rest[..end];
        let mut afrom = 0;
        while let Some(j) = attrs[afrom..].find(attr) {
            let abs = afrom + j;
            let before_ok = abs == 0
                || attrs[..abs]
                    .chars()
                    .rev()
                    .next()
                    .is_some_and(|c| c.is_whitespace() || c == '/');
            let after_ok = attrs[abs + attr.len()..]
                .chars()
                .next()
                .map(|c| c.is_whitespace() || c == '=' || c == '>' || c == '/')
                .unwrap_or(true);
            if before_ok && after_ok {
                return true;
            }
            afrom = abs + attr.len();
        }
        from = after + end + 1;
    }
    false
}

/// Links, images, tables, lists, headings — not Chromium's styled-span wrapper.
fn html_has_rich_markup(html: &str) -> bool {
    let l = html.to_ascii_lowercase();
    if open_tag_has_attr(&l, "a", "href") {
        return true;
    }
    [
        "img", "table", "ul", "ol", "h1", "h2", "h3", "h4", "h5", "h6", "video", "audio", "iframe",
        "object", "svg",
    ]
    .iter()
    .any(|t| has_open_tag(&l, t))
}

fn html_is_plain_wrapper(html: &str, plain: &str) -> bool {
    if html_has_rich_markup(html) {
        return false;
    }
    let visible = html_visible_text(html);
    if visible.trim().is_empty() {
        return false;
    }
    normalize_visible(&visible) == normalize_visible(plain)
}

fn rtf_source(rtf: &[u8]) -> String {
    let end = rtf
        .iter()
        .rposition(|&b| b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
    let bytes = &rtf[..end];
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().copied().map(char::from).collect(),
    }
}

/// Images, tables, links, embedded objects — not a font/color wrapper.
fn rtf_has_rich_markup(rtf: &[u8]) -> bool {
    let s = rtf_source(rtf).to_ascii_lowercase();
    s.contains("\\pict")
        || s.contains("\\trowd")
        || s.contains("\\cell")
        || s.contains("hyperlink")
        || s.contains("\\object")
        || s.contains("\\shp")
}

fn rtf_dest_skip(word: &str) -> bool {
    matches!(
        word,
        "fonttbl"
            | "colortbl"
            | "stylesheet"
            | "info"
            | "pict"
            | "object"
            | "header"
            | "footer"
            | "headerf"
            | "footerf"
            | "footnote"
            | "annotation"
            | "fldinst"
            | "datafield"
            | "generator"
            | "listtable"
            | "listoverridetable"
            | "rsidtbl"
            | "themedata"
            | "colorschememapping"
            | "latentstyles"
            | "xmlnstbl"
            | "filetbl"
            | "mmath"
            | "shpinst"
            | "blipuid"
            | "xe"
            | "tc"
    )
}

fn rtf_visible_text(rtf: &[u8]) -> String {
    let src = rtf_source(rtf);
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut skip_until: Option<i32> = None;
    let mut uc: usize = 1;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                if skip_until == Some(depth) {
                    skip_until = None;
                }
                depth = (depth - 1).max(0);
                i += 1;
            }
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                let n = chars[i];
                if n == '\\' || n == '{' || n == '}' {
                    if skip_until.is_none() {
                        out.push(n);
                    }
                    i += 1;
                    continue;
                }
                if n == '\'' {
                    i += 1;
                    let hex: String = chars[i..].iter().take(2).collect();
                    if hex.len() == 2 && hex.chars().all(|h| h.is_ascii_hexdigit()) {
                        i += 2;
                        if skip_until.is_none() {
                            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                                out.push(char::from(b));
                            }
                        }
                    }
                    continue;
                }
                if n == '*' {
                    skip_until = Some(depth.max(1));
                    i += 1;
                    continue;
                }
                if n == '~' {
                    if skip_until.is_none() {
                        out.push('\u{00a0}');
                    }
                    i += 1;
                    continue;
                }
                if n == '_' {
                    if skip_until.is_none() {
                        out.push('-');
                    }
                    i += 1;
                    continue;
                }
                if n == '-' {
                    i += 1;
                    continue;
                }
                if !n.is_ascii_alphabetic() {
                    i += 1;
                    continue;
                }
                let start = i;
                i += 1;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let mut neg = false;
                if chars.get(i) == Some(&'-') {
                    neg = true;
                    i += 1;
                }
                let num_start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let num = if i > num_start {
                    let raw: String = chars[num_start..i].iter().collect();
                    raw.parse::<i32>().ok().map(|n| if neg { -n } else { n })
                } else {
                    None
                };
                if chars.get(i) == Some(&' ') {
                    i += 1;
                }
                if rtf_dest_skip(&word) {
                    skip_until = Some(depth.max(1));
                }
                if skip_until.is_some() {
                    continue;
                }
                match word.as_str() {
                    "par" | "line" | "row" | "page" => out.push('\n'),
                    "tab" => out.push('\t'),
                    "emdash" => out.push('—'),
                    "endash" => out.push('–'),
                    "lquote" | "rquote" => out.push('\''),
                    "ldblquote" | "rdblquote" => out.push('"'),
                    "bullet" => out.push('•'),
                    "uc" => {
                        if let Some(n) = num {
                            uc = n.max(0) as usize;
                        }
                    }
                    "u" => {
                        if let Some(n) = num {
                            let cp = if n < 0 {
                                (n as i32 + 65536) as u32
                            } else {
                                n as u32
                            };
                            if let Some(ch) = char::from_u32(cp) {
                                out.push(ch);
                            }
                        }
                        let mut left = uc;
                        while left > 0 && i < chars.len() {
                            if chars[i] == '\\' && chars.get(i + 1) == Some(&'\'') {
                                i += 2;
                                i += chars[i..]
                                    .iter()
                                    .take(2)
                                    .take_while(|h| h.is_ascii_hexdigit())
                                    .count();
                            } else {
                                i += 1;
                            }
                            left -= 1;
                        }
                    }
                    _ => {}
                }
            }
            '\n' | '\r' => i += 1,
            _ => {
                if skip_until.is_none() {
                    out.push(c);
                }
                i += 1;
            }
        }
    }
    out
}

fn empty_captured() -> CapturedItem {
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

/// Chromium / Electron / Word put styled HTML and RTF beside the same unicode text.
/// Keep structurally rich formats; drop font/color wrappers so history is text.
fn normalize_captured(captured: &CapturedPasteboard) -> CapturedPasteboard {
    let mut plain = captured.items.iter().find_map(|i| {
        (i.ty == PasteType::Text)
            .then(|| i.text.clone())
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    });

    let mut items = Vec::with_capacity(captured.items.len());
    let mut converted: Option<String> = None;
    for item in &captured.items {
        match item.ty {
            PasteType::Html => {
                let Some(html) = item.html.as_deref() else {
                    items.push(item.clone());
                    continue;
                };
                if html_has_rich_markup(html) {
                    items.push(item.clone());
                    continue;
                }
                let visible = html_visible_text(html);
                let drop = match plain.as_deref() {
                    Some(p) => html_is_plain_wrapper(html, p),
                    None => !visible.trim().is_empty(),
                };
                if drop {
                    remember_converted(&mut converted, &mut plain, visible);
                } else {
                    items.push(item.clone());
                }
            }
            PasteType::Rtf => {
                let Some(rtf) = item.rtf.as_deref() else {
                    items.push(item.clone());
                    continue;
                };
                if rtf_has_rich_markup(rtf) {
                    items.push(item.clone());
                    continue;
                }
                if plain.is_some() {
                    continue;
                }
                let visible = rtf_visible_text(rtf);
                if !visible.trim().is_empty() {
                    remember_converted(&mut converted, &mut plain, visible);
                } else {
                    items.push(item.clone());
                }
            }
            _ => items.push(item.clone()),
        }
    }
    if let Some(text) = converted {
        if items.iter().all(|i| i.ty != PasteType::Text) {
            items.push(CapturedItem {
                ty: PasteType::Text,
                text: Some(text),
                ..empty_captured()
            });
        }
    }
    CapturedPasteboard { items }
}

fn remember_converted(converted: &mut Option<String>, plain: &mut Option<String>, visible: String) {
    if plain.is_none() && !visible.trim().is_empty() {
        *plain = Some(visible.trim().to_string());
        if converted.is_none() {
            *converted = Some(visible);
        }
    }
}

fn html_title(html: &str, sidecar_text: Option<&str>) -> String {
    if let Some(t) = sidecar_text.map(str::trim).filter(|s| !s.is_empty()) {
        return truncate_title(t, 80);
    }
    let visible = html_visible_text(html);
    if !visible.trim().is_empty() {
        truncate_title(&visible, 80)
    } else {
        truncate_title(html, 80)
    }
}

pub fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((w, h))
}

const IMAGE_THUMB_EDGE: u32 = 96;
const INLINE_IMAGE_MAX: usize = 80 * 1024;
const IMAGE_THUMB_MAX_FILE: u64 = 40 * 1024 * 1024;
const IMAGE_FILE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "tif", "tiff", "bmp", "webp", "ico", "heic", "heif", "svg",
];

pub fn image_dimensions_any(data: &[u8]) -> Option<(u32, u32)> {
    png_dimensions(data).or_else(|| {
        ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok()
    })
}

pub fn looks_like_image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_FILE_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(false)
}

pub fn looks_like_image_magic(bytes: &[u8]) -> bool {
    sniff_image_mime_opt(bytes).is_some()
}

pub fn file_looks_like_image(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(looks_like_image_name)
    {
        return true;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 16];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    looks_like_image_magic(&buf[..n])
}

fn sniff_image_mime_opt(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 8 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some("image/jpeg")
    } else if bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        Some("image/gif")
    } else if bytes.len() >= 4
        && ((bytes[0] == 0x49 && bytes[1] == 0x49 && bytes[2] == 0x2A && bytes[3] == 0x00)
            || (bytes[0] == 0x4D && bytes[1] == 0x4D && bytes[2] == 0x00 && bytes[3] == 0x2A))
    {
        Some("image/tiff")
    } else if bytes.len() >= 2 && bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    sniff_image_mime_opt(bytes).unwrap_or("image/png")
}

pub fn make_image_thumb_data_url(bytes: &[u8]) -> Option<String> {
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let thumb = img.thumbnail(IMAGE_THUMB_EDGE, IMAGE_THUMB_EDGE);
    encode_thumb_data_url(&thumb)
}

fn encode_thumb_data_url(img: &DynamicImage) -> Option<String> {
    let mut buf = Vec::new();
    if img.color().has_alpha() {
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .ok()?;
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buf)
        ))
    } else {
        DynamicImage::ImageRgb8(img.to_rgb8())
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
            .ok()?;
        Some(format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buf)
        ))
    }
}

/// Compact list thumbnail + dimensions. Does not rewrite `preview.path`.
pub fn apply_image_thumb(preview: &mut Preview, bytes: &[u8]) {
    if let Some((w, h)) = image_dimensions_any(bytes) {
        preview.width = Some(w);
        preview.height = Some(h);
    }
    if (bytes.len() as u64) > IMAGE_THUMB_MAX_FILE {
        return;
    }
    preview.image_thumb = make_image_thumb_data_url(bytes).or_else(|| {
        if bytes.len() <= INLINE_IMAGE_MAX {
            Some(format!(
                "data:{};base64,{}",
                sniff_image_mime(bytes),
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ))
        } else {
            preview.path.clone()
        }
    });
}

pub fn apply_image_thumb_from_path(preview: &mut Preview, path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() == 0 {
        return;
    }
    if meta.len() > IMAGE_THUMB_MAX_FILE {
        return;
    }
    match ImageReader::open(path)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.decode().ok())
    {
        Some(img) => {
            preview.width = Some(img.width());
            preview.height = Some(img.height());
            preview.image_thumb =
                encode_thumb_data_url(&img.thumbnail(IMAGE_THUMB_EDGE, IMAGE_THUMB_EDGE))
                    .or_else(|| preview.path.clone());
        }
        None => {
            if looks_like_image_name(&path.to_string_lossy()) {
                preview.image_thumb = preview.path.clone();
            }
        }
    }
}

/// List thumbnail plus a filesystem path the overlay can load for the full preview.
pub fn apply_image_preview(store: &Store, preview: &mut Preview, hash: &str, bytes: &[u8]) {
    let name = match sniff_image_mime(bytes) {
        "image/jpeg" => "image.jpg",
        "image/gif" => "image.gif",
        "image/tiff" => "image.tiff",
        "image/bmp" => "image.bmp",
        "image/webp" => "image.webp",
        _ => "image.png",
    };
    let path = store
        .named_blob_path(hash, Some(name))
        .unwrap_or_else(|_| store.blob_path(hash))
        .to_string_lossy()
        .into_owned();
    preview.path = Some(path);
    apply_image_thumb(preview, bytes);
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
    locale_pref: &str,
) -> Result<Option<IngestResult>, String> {
    let captured = normalize_captured(captured);
    if captured.items.is_empty() {
        return Ok(None);
    }
    let hash = captured.content_hash();
    if store.should_skip_duplicate_hash(&hash)? {
        return Ok(None);
    }

    let locale = crate::i18n::resolve(locale_pref);
    let id = uuid::Uuid::new_v4().to_string();
    let ptype = primary_type(&captured.types());
    let sidecar_text = captured.items.iter().find_map(|i| {
        (i.ty == PasteType::Text)
            .then(|| i.text.as_deref())
            .flatten()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    });
    let mut preview = Preview::default();
    let mut items: Vec<StoredItem> = Vec::new();
    let mut total_bytes = 0u64;
    let mut title = crate::i18n::t(locale, "overlay.genericClipboard");

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
                    let dest = store
                        .named_blob_path(&hash, Some(&name))
                        .unwrap_or_else(|_| store.blob_path(&hash));
                    preview.path = Some(dest.to_string_lossy().into_owned());
                    if file_looks_like_image(path) {
                        apply_image_thumb_from_path(&mut preview, &dest);
                        stored.width = preview.width;
                        stored.height = preview.height;
                    }
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
                    // Every downloadable item carries a token so GET /files can
                    // bind the request to the item, not just to any paired peer.
                    stored.download_token = Some(hex::encode(rand_bytes(16)));
                    if let Some((w, h)) = image_dimensions_any(bytes) {
                        stored.width = Some(w);
                        stored.height = Some(h);
                    }
                    apply_image_preview(store, &mut preview, &hash, bytes);
                    if ptype == PasteType::Image {
                        title = crate::i18n::t(locale, "type.image");
                    }
                    total_bytes += size;
                }
            }
            PasteType::Html => {
                if let Some(html) = &cap.html {
                    let visible = html_visible_text(html);
                    preview.html = Some(truncate_title(
                        if visible.trim().is_empty() {
                            html
                        } else {
                            &visible
                        },
                        400,
                    ));
                    if ptype == PasteType::Html {
                        title = html_title(html, sidecar_text);
                    }
                    total_bytes += html.len() as u64;
                }
            }
            PasteType::Rtf => {
                if let Some(rtf) = &cap.rtf {
                    let visible = rtf_visible_text(rtf);
                    if ptype == PasteType::Rtf {
                        title = sidecar_text
                            .map(|t| truncate_title(t, 80))
                            .filter(|t| !t.is_empty())
                            .or_else(|| {
                                let t = visible.trim();
                                (!t.is_empty()).then(|| truncate_title(t, 80))
                            })
                            .unwrap_or_else(|| "RTF".into());
                        if preview.text.is_none() && !visible.trim().is_empty() {
                            preview.text = Some(truncate_title(&visible, 400));
                        }
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

impl WritePayload {
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.html.is_none()
            && self.rtf.is_none()
            && self.url.is_none()
            && self.image_png.is_none()
            && self.file_paths.is_empty()
    }
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
    SKIP_CLIPBOARD_READ.store(true, Ordering::SeqCst);
    let result = {
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
            Ok(0i64)
        }
    };
    SKIP_CLIPBOARD_READ.store(false, Ordering::SeqCst);
    match &result {
        Ok(count) => mark_own_write(*count),
        Err(_) => note_own_promise(None),
    }
    result
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
    use std::sync::Mutex;

    use objc2::runtime::ProtocolObject;
    #[allow(deprecated)]
    use objc2_app_kit::NSFilenamesPboardType;
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
            let n = app
                .localizedName()
                .map(|s| s.to_string())
                .unwrap_or_default();
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
        let skip_files = super::own_promise_active();

        if !skip_files {
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
                writers.push(ProtocolObject::<dyn NSPasteboardWriting>::from_retained(
                    url,
                ));
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

    use objc2::rc::Retained;
    use objc2::{define_class, msg_send, AnyThread, DefinedClass};
    use objc2_app_kit::{NSPasteboardItem, NSPasteboardItemDataProvider, NSPasteboardType};
    use objc2_foundation::{NSObject, NSObjectProtocol};

    struct FilePromiseIvars {
        pasteboard_id: String,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "LanPasteFilePromiseProvider"]
        #[ivars = FilePromiseIvars]
        struct FilePromiseProvider;

        unsafe impl NSObjectProtocol for FilePromiseProvider {}

        unsafe impl NSPasteboardItemDataProvider for FilePromiseProvider {
            #[allow(non_snake_case)]
            #[unsafe(method(pasteboard:item:provideDataForType:))]
            fn pasteboard_item_provideDataForType(
                &self,
                _pasteboard: Option<&NSPasteboard>,
                item: &NSPasteboardItem,
                r#type: &NSPasteboardType,
            ) {
                let Ok(path) = super::fulfill_promised_file_id(&self.ivars().pasteboard_id) else {
                    return;
                };
                let path_s = NSString::from_str(&path.to_string_lossy());
                unsafe {
                    let is_file_url = r#type == NSPasteboardTypeFileURL;
                    #[allow(deprecated)]
                    let is_filenames = r#type == NSFilenamesPboardType;
                    if is_file_url {
                        let url = NSURL::fileURLWithPath_isDirectory(&path_s, path.is_dir());
                        if let Some(abs) = url.absoluteString() {
                            let _ = item.setString_forType(&abs, NSPasteboardTypeFileURL);
                        }
                    } else if is_filenames {
                        let arr = NSArray::from_slice(&[&*path_s]);
                        #[allow(deprecated)]
                        let _ = item.setPropertyList_forType(&arr, NSFilenamesPboardType);
                    }
                }
                let _ = self.ivars().pasteboard_id;
            }
        }
    );

    impl FilePromiseProvider {
        fn new(pasteboard_id: String) -> Retained<Self> {
            let this = Self::alloc().set_ivars(FilePromiseIvars { pasteboard_id });
            unsafe { msg_send![super(this), init] }
        }
    }

    static MAC_PROVIDER: Mutex<Option<Retained<FilePromiseProvider>>> = Mutex::new(None);

    pub fn write_promise(pasteboard_id: &str) -> Result<i64, String> {
        let provider = FilePromiseProvider::new(pasteboard_id.to_string());
        let item = NSPasteboardItem::new();
        #[allow(deprecated)]
        let types =
            unsafe { NSArray::from_slice(&[NSPasteboardTypeFileURL, NSFilenamesPboardType]) };
        let proto = ProtocolObject::<dyn NSPasteboardItemDataProvider>::from_ref(&*provider);
        if !item.setDataProvider_forTypes(&proto, &types) {
            return Err("无法声明延迟文件".into());
        }
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let writer = ProtocolObject::<dyn NSPasteboardWriting>::from_retained(item);
        if !pb.writeObjects(&NSArray::from_retained_slice(&[writer])) {
            return Err("无法写入延迟文件剪贴板".into());
        }
        if let Ok(mut g) = MAC_PROVIDER.lock() {
            *g = Some(provider);
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
    use windows::Win32::Foundation::{
        CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_V,
    };
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, CallWindowProcW, FindWindowExW, FindWindowW,
        GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
        IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, SetWindowLongPtrW, ShowWindow,
        SwitchToThisWindow, ASFW_ANY, GA_ROOT, GWLP_WNDPROC, SW_RESTORE, WNDPROC,
    };

    struct PrevTarget {
        hwnd: isize,
        name: String,
    }

    static PREV: Mutex<PrevTarget> = Mutex::new(PrevTarget {
        hwnd: 0,
        name: String::new(),
    });

    const WM_RENDERFORMAT: u32 = 0x0306;
    const WM_RENDERALLFORMATS: u32 = 0x0307;
    const WM_DESTROYCLIPBOARD: u32 = 0x0308;

    static OWNER_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
    static ORIG_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

    pub fn write_delayed_hdrop() -> Result<i64, String> {
        let owner = hwnd_from_isize(OWNER_HWND.load(std::sync::atomic::Ordering::SeqCst));
        if hwnd_null(owner) {
            return Err("剪贴板窗口未就绪".into());
        }
        unsafe {
            let mut opened = false;
            for _ in 0..8 {
                if OpenClipboard(Some(owner)).is_ok() {
                    opened = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            if !opened {
                return Err("无法打开剪贴板".into());
            }
            let result = (|| {
                EmptyClipboard().map_err(|e| e.to_string())?;
                SetClipboardData(u32::from(CF_HDROP.0), None).map_err(|e| e.to_string())?;
                Ok(GetClipboardSequenceNumber() as i64)
            })();
            let _ = CloseClipboard();
            result
        }
    }

    pub fn subclass_owner(hwnd_val: isize) {
        let hwnd = hwnd_from_isize(hwnd_val);
        if hwnd_null(hwnd) {
            return;
        }
        OWNER_HWND.store(hwnd_val, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, delayed_wndproc as usize as isize);
            ORIG_WNDPROC.store(prev, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn render_hdrop() {
        let Ok(path) = super::fulfill_promised_file() else {
            return;
        };
        let _ = unsafe { set_clipboard_bytes(u32::from(CF_HDROP.0), &hdrop_bytes(&[path])) };
    }

    unsafe extern "system" fn delayed_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let render = WM_RENDERFORMAT;
        let render_all = WM_RENDERALLFORMATS;
        let destroy = WM_DESTROYCLIPBOARD;
        if msg == render {
            if wparam.0 as u32 == u32::from(CF_HDROP.0) {
                render_hdrop();
            }
            return LRESULT(0);
        }
        if msg == render_all {
            if OpenClipboard(Some(hwnd)).is_ok() {
                let _ = EmptyClipboard();
                render_hdrop();
                let _ = CloseClipboard();
            }
            return LRESULT(0);
        }
        if msg == destroy {
            super::note_own_promise(None);
        }
        let orig = ORIG_WNDPROC.load(std::sync::atomic::Ordering::SeqCst);
        if orig == 0 {
            return LRESULT(0);
        }
        let proc = std::mem::transmute::<isize, WNDPROC>(orig);
        CallWindowProcW(proc, hwnd, msg, wparam, lparam)
    }

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

    fn is_tray_window(hwnd: HWND) -> bool {
        matches!(
            class_name(hwnd).as_str(),
            "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "NotifyIconOverflowWindow"
                | "ForegroundStaging"
                | "Windows.Internal.Shell.TabProxyWindow"
        )
    }

    /// Top-level hosts for the wallpaper / icon desktop — not Explorer folders
    /// (`CabinetWClass`) and not arbitrary `SysListView32` controls.
    fn is_desktop_host_class(class: &str) -> bool {
        matches!(class, "Progman" | "WorkerW" | "#32769")
    }

    fn is_desktop_hwnd(hwnd: HWND) -> bool {
        is_desktop_host_class(&class_name(hwnd))
            || is_desktop_host_class(&class_name(top_level(hwnd)))
    }

    fn find_child(parent: Option<HWND>, after: Option<HWND>, class: windows::core::PCWSTR) -> HWND {
        unsafe {
            FindWindowExW(parent, after, class, windows::core::PCWSTR::null())
                .unwrap_or(HWND(std::ptr::null_mut()))
        }
    }

    fn defview_list(defview: HWND) -> HWND {
        let list = find_child(Some(defview), None, w!("SysListView32"));
        if !hwnd_null(list) {
            return list;
        }
        let dui = find_child(Some(defview), None, w!("DirectUIHWND"));
        if hwnd_null(dui) {
            defview
        } else {
            dui
        }
    }

    /// Desktop icon list — Ctrl+V here drops files onto the desktop.
    fn desktop_paste_hwnd() -> Option<HWND> {
        unsafe {
            let progman = FindWindowW(w!("Progman"), windows::core::PCWSTR::null())
                .unwrap_or(HWND(std::ptr::null_mut()));
            let mut defview = find_child(
                Some(progman).filter(|h| !hwnd_null(*h)),
                None,
                w!("SHELLDLL_DefView"),
            );
            if hwnd_null(defview) {
                let mut after: Option<HWND> = None;
                for _ in 0..32 {
                    let worker = find_child(None, after, w!("WorkerW"));
                    if hwnd_null(worker) {
                        break;
                    }
                    defview = find_child(Some(worker), None, w!("SHELLDLL_DefView"));
                    if !hwnd_null(defview) {
                        break;
                    }
                    after = Some(worker);
                }
            }
            if hwnd_null(defview) {
                return None;
            }
            Some(defview_list(defview))
        }
    }

    fn resolve_target(fg: HWND) -> Option<(HWND, String)> {
        if hwnd_null(fg) || is_own_process(fg) || is_tray_window(fg) {
            return None;
        }
        if is_desktop_hwnd(fg) {
            let hwnd = desktop_paste_hwnd().unwrap_or(top_level(fg));
            return Some((hwnd, "桌面".into()));
        }
        let hwnd = top_level(fg);
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
                return None;
            }
        }
        if is_own_process(hwnd) || is_tray_window(hwnd) {
            return None;
        }
        Some((hwnd, process_name(hwnd)))
    }

    pub fn track_frontmost() {
        let fg = unsafe { GetForegroundWindow() };
        if let Some((hwnd, name)) = resolve_target(fg) {
            let mut g = prev_lock();
            g.hwnd = hwnd_as_isize(hwnd);
            g.name = name;
        }
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
        if let Some((_, name)) = resolve_target(hwnd) {
            return Ok(name);
        }
        Ok(prev_lock().name.clone())
    }

    pub fn change_count() -> Result<i64, String> {
        Ok(unsafe { GetClipboardSequenceNumber() as i64 })
    }

    fn force_foreground(hwnd: HWND) {
        unsafe {
            if hwnd_null(hwnd) || !IsWindow(Some(hwnd)).as_bool() {
                return;
            }
            let desktop = is_desktop_hwnd(hwnd);
            // SetForegroundWindow only works on top-level windows. The desktop
            // paste target is the icon list (child); raise Progman/WorkerW instead.
            let raise = if desktop {
                let top = top_level(hwnd);
                if hwnd_null(top) {
                    hwnd
                } else {
                    top
                }
            } else {
                hwnd
            };
            if IsIconic(raise).as_bool() {
                let _ = ShowWindow(raise, SW_RESTORE);
            }

            let fg = GetForegroundWindow();
            if !desktop && !hwnd_null(fg) && hwnd_as_isize(fg) == hwnd_as_isize(hwnd) {
                return;
            }

            // Attach the *current foreground* thread to the target, not this worker.
            let fg_tid = if hwnd_null(fg) {
                0
            } else {
                GetWindowThreadProcessId(fg, None)
            };
            let target_tid = GetWindowThreadProcessId(raise, None);
            let attached = fg_tid != 0
                && target_tid != 0
                && fg_tid != target_tid
                && AttachThreadInput(fg_tid, target_tid, true).as_bool();

            let _ = AllowSetForegroundWindow(ASFW_ANY);
            if !desktop {
                SwitchToThisWindow(raise, true);
                let _ = BringWindowToTop(raise);
            }
            let _ = SetForegroundWindow(raise);
            if desktop {
                let _ = SetFocus(Some(hwnd));
            }

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
        let body =
            format!("<html>\r\n<body>\r\n{start_frag}{html}{end_frag}\r\n</body>\r\n</html>");
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
            let text = payload.text.clone().or_else(|| payload.url.clone());
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
        if let (Some(a), Some(b)) = (s.find("<!--StartFragment-->"), s.find("<!--EndFragment-->")) {
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
            let skip_files = super::own_promise_active();
            unsafe {
                if !skip_files && IsClipboardFormatAvailable(u32::from(CF_HDROP.0)).is_ok() {
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

    fn cap_html(html: &str) -> CapturedItem {
        CapturedItem {
            ty: PasteType::Html,
            text: None,
            html: Some(html.into()),
            rtf: None,
            url: None,
            color: None,
            image: None,
            file_path: None,
        }
    }

    fn cap_rtf(rtf: &str) -> CapturedItem {
        CapturedItem {
            ty: PasteType::Rtf,
            text: None,
            html: None,
            rtf: Some(rtf.as_bytes().to_vec()),
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
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let pb = store.get_pasteboard(&result.id).unwrap().unwrap();
        assert_eq!(pb.primary_type, "file");
        assert_eq!(pb.title, "新增 文本文档.txt");
        let preview: crate::types::Preview = serde_json::from_str(&pb.preview_json).unwrap();
        assert_eq!(preview.file_name.as_deref(), Some("新增 文本文档.txt"));
        assert!(preview.text.is_none());
        let payload = payload_from_store(&store, &result.id).unwrap();
        assert_eq!(
            payload.file_paths[0].file_name().unwrap().to_string_lossy(),
            "新增 文本文档.txt"
        );
        assert!(payload.text.is_none());
    }

    fn cap_image(bytes: Vec<u8>) -> CapturedItem {
        CapturedItem {
            ty: PasteType::Image,
            text: None,
            html: None,
            rtf: None,
            url: None,
            color: None,
            image: Some(bytes),
            file_path: None,
        }
    }

    fn png_bytes(w: u32, h: u32, noisy: bool) -> Vec<u8> {
        let mut img = image::RgbImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            if noisy {
                let n = x
                    .wrapping_mul(1_103_515_245)
                    .wrapping_add(y.wrapping_mul(12_345));
                *p = image::Rgb([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
            } else {
                *p = image::Rgb([(x % 256) as u8, (y % 256) as u8, 80]);
            }
        }
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn image_ingest_stores_data_url_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let small = png_bytes(16, 16, false);
        let captured = CapturedPasteboard {
            items: vec![cap_image(small)],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        let thumb = entry.preview.image_thumb.expect("thumb");
        assert!(thumb.starts_with("data:image/"));
        assert_eq!(entry.preview.width, Some(16));
        assert_eq!(entry.preview.height, Some(16));
        assert!(entry.preview.path.is_some());
    }

    #[test]
    fn large_image_still_gets_compact_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let large = png_bytes(512, 512, true);
        assert!(
            large.len() > INLINE_IMAGE_MAX,
            "fixture too small: {}",
            large.len()
        );
        let captured = CapturedPasteboard {
            items: vec![cap_image(large)],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        let thumb = entry.preview.image_thumb.expect("thumb");
        assert!(thumb.starts_with("data:image/"));
        assert!(thumb.len() < INLINE_IMAGE_MAX);
    }

    #[test]
    fn image_file_ingest_keeps_file_type_and_gets_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let src = dir.path().join("截图.png");
        std::fs::write(&src, png_bytes(32, 24, false)).unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_file(src), cap_text("/tmp/截图.png")],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::File);
        assert_eq!(entry.title, "截图.png");
        assert_eq!(entry.preview.file_name.as_deref(), Some("截图.png"));
        let thumb = entry.preview.image_thumb.expect("thumb");
        assert!(thumb.starts_with("data:image/"));
        assert_eq!(entry.preview.width, Some(32));
        assert_eq!(entry.preview.height, Some(24));
        let payload = payload_from_store(&store, &result.id).unwrap();
        assert_eq!(
            payload.file_paths[0].file_name().unwrap().to_string_lossy(),
            "截图.png"
        );
        assert!(payload.image_png.is_none());
    }

    #[test]
    fn plain_file_ingest_has_no_image_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let src = dir.path().join("notes.txt");
        std::fs::write(&src, b"hello").unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_file(src)],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::File);
        assert!(entry.preview.image_thumb.is_none());
        assert!(entry.preview.width.is_none());
    }

    #[test]
    fn looks_like_image_name_matches_common_exts() {
        assert!(looks_like_image_name("a.PNG"));
        assert!(looks_like_image_name("photo.jpeg"));
        assert!(looks_like_image_name("x.webp"));
        assert!(!looks_like_image_name("notes.txt"));
        assert!(!looks_like_image_name("archive.tar.gz"));
        assert!(looks_like_image_magic(&png_bytes(8, 8, false)));
        assert!(!looks_like_image_magic(b"hello"));
    }

    const CHROMIUM_HTML: &str = r#"<meta charset="UTF-8"><span style="caret-color: rgb(0, 0, 0); color: rgb(0, 0, 0); font-style: normal;">管理修改账户信息</span>"#;

    #[test]
    fn chromium_wrapper_visible_text() {
        assert_eq!(html_visible_text(CHROMIUM_HTML).trim(), "管理修改账户信息");
        assert!(html_is_plain_wrapper(CHROMIUM_HTML, "管理修改账户信息"));
        assert!(!html_has_rich_markup(CHROMIUM_HTML));
    }

    #[test]
    fn chromium_wrapper_ingest_as_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_html(CHROMIUM_HTML), cap_text("管理修改账户信息")],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::Text);
        assert_eq!(entry.title, "管理修改账户信息");
        let payload = payload_from_store(&store, &result.id).unwrap();
        assert_eq!(payload.text.as_deref(), Some("管理修改账户信息"));
        assert!(payload.html.is_none());
    }

    #[test]
    fn chromium_html_only_becomes_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_html(CHROMIUM_HTML)],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::Text);
        assert_eq!(entry.title, "管理修改账户信息");
    }

    #[test]
    fn linked_html_stays_html_with_text_title() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let html = r#"see <a href="https://example.com">here</a>"#;
        let captured = CapturedPasteboard {
            items: vec![cap_html(html), cap_text("see here")],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::Html);
        assert_eq!(entry.title, "see here");
        let payload = payload_from_store(&store, &result.id).unwrap();
        assert_eq!(payload.html.as_deref(), Some(html));
        assert_eq!(payload.text.as_deref(), Some("see here"));
    }

    #[test]
    fn vscode_highlighted_copy_is_text() {
        let html = r#"<meta charset="utf-8"><div style="color: #d4d4d4;font-family: Consolas"><span style="color: #569cd6;">fn</span><span> main() {}</span></div>"#;
        assert!(html_is_plain_wrapper(html, "fn main() {}"));
        let n = normalize_captured(&CapturedPasteboard {
            items: vec![cap_html(html), cap_text("fn main() {}")],
        });
        assert!(n.items.iter().all(|i| i.ty != PasteType::Html));
        assert_eq!(primary_type(&n.types()), PasteType::Text);
    }

    const COCOA_RTF: &str = r#"{\rtf1\ansi\ansicpg1252\cocoartf2822
{\fonttbl\f0\fnil\fcharset0 HelveticaNeue;}
{\colortbl;\red255\green255\blue255;\red0\green0\blue0;}
\pard\tx560\pardirnatural\partightenfactor0

\f0\fs28 \cf2 hello world}"#;

    const UNICODE_RTF: &str = r#"{\rtf1\ansi\ansicpg936\cocoartf2822
{\fonttbl\f0\fnil\fcharset134 PingFangSC-Regular;}
{\colortbl;\red255\green255\blue255;}
\pard\tx560\pardirnatural\partightenfactor0

\f0\fs28 \cf0 \u31649?\u29702?\u20462?\u25913?\u36134?\u25143?\u20449?\u24687?}"#;

    #[test]
    fn cocoa_rtf_visible_text() {
        assert_eq!(
            normalize_visible(&rtf_visible_text(COCOA_RTF.as_bytes())),
            "hello world"
        );
        assert_eq!(
            normalize_visible(&rtf_visible_text(UNICODE_RTF.as_bytes())),
            "管理修改账户信息"
        );
        assert!(!rtf_has_rich_markup(COCOA_RTF.as_bytes()));
    }

    #[test]
    fn rtf_wrapper_with_text_ingest_as_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let captured = CapturedPasteboard {
            items: vec![
                cap_html(CHROMIUM_HTML),
                cap_rtf(UNICODE_RTF),
                cap_text("管理修改账户信息"),
            ],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::Text);
        assert_eq!(entry.title, "管理修改账户信息");
        let payload = payload_from_store(&store, &result.id).unwrap();
        assert_eq!(payload.text.as_deref(), Some("管理修改账户信息"));
        assert!(payload.html.is_none());
        assert!(payload.rtf.is_none());
    }

    #[test]
    fn rtf_only_wrapper_becomes_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_rtf(COCOA_RTF)],
        };
        let result = ingest_captured(&mut store, &captured, "本机", "dev", "zh-CN")
            .unwrap()
            .unwrap();
        let entry = store.get_entry(&result.id).unwrap().unwrap();
        assert_eq!(entry.primary_type, crate::types::PasteType::Text);
        assert_eq!(entry.title, "hello world");
    }

    #[test]
    fn rtf_with_picture_stays_rtf() {
        let rtf = r#"{\rtf1\ansi{\fonttbl\f0\fnil Helvetica;}{\pict\pngblip 89504e47}hello}"#;
        assert!(rtf_has_rich_markup(rtf.as_bytes()));
        let n = normalize_captured(&CapturedPasteboard {
            items: vec![cap_rtf(rtf), cap_text("hello")],
        });
        assert!(n.items.iter().any(|i| i.ty == PasteType::Rtf));
        assert_eq!(primary_type(&n.types()), PasteType::Rtf);
    }

    #[test]
    fn ingest_skips_consecutive_duplicate_hash() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_text("hello echo")],
        };
        assert!(ingest_captured(&mut store, &captured, "本机", "a", "zh-CN")
            .unwrap()
            .is_some());
        assert!(ingest_captured(&mut store, &captured, "本机", "a", "zh-CN")
            .unwrap()
            .is_none());
    }

    #[test]
    fn ingest_skips_when_latest_is_remote_with_same_hash() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let captured = CapturedPasteboard {
            items: vec![cap_text("sync echo")],
        };
        let hash = captured.content_hash();
        let pb = crate::store::StoredPasteboard {
            id: "remote-1".into(),
            copied_at: 1,
            source_device_id: Some("peer".into()),
            source_device_name: "Peer".into(),
            primary_type: "text".into(),
            title: "sync echo".into(),
            content_hash: hash,
            total_bytes: 9,
            needs_file_download: false,
            file_download_state: "idle".into(),
            download_token: None,
            source_host: None,
            source_port: None,
            preview_json: "{}".into(),
        };
        store.insert_pasteboard(&pb, &[]).unwrap();
        assert!(ingest_captured(&mut store, &captured, "本机", "a", "zh-CN")
            .unwrap()
            .is_none());
    }

    #[test]
    fn own_write_grace_ignores_extra_change_counts() {
        mark_own_write(42);
        assert!(own_write_in_grace());
        assert!(should_ignore_own_change(99, -1));
        assert!(should_ignore_own_change(42, 42));
    }
}
