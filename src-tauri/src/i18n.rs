use std::collections::HashMap;
use std::sync::OnceLock;

use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const ZH_CN_JSON: &str = include_str!("../../src/locales/zh-CN.json");
const EN_US_JSON: &str = include_str!("../../src/locales/en-US.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocaleId {
    EnUs,
    ZhCn,
}

impl LocaleId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }
}

pub fn normalize_pref(value: &str) -> String {
    match value {
        "en-US" | "zh-CN" | "system" => value.to_string(),
        _ => "system".into(),
    }
}

pub fn detect_system() -> LocaleId {
    match sys_locale::get_locale() {
        Some(s) if s.to_ascii_lowercase().starts_with("zh") => LocaleId::ZhCn,
        _ => LocaleId::EnUs,
    }
}

pub fn resolve(pref: &str) -> LocaleId {
    match pref {
        "en-US" => LocaleId::EnUs,
        "zh-CN" => LocaleId::ZhCn,
        _ => detect_system(),
    }
}

fn parse_catalog(raw: &str) -> HashMap<String, String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn catalog(locale: LocaleId) -> &'static HashMap<String, String> {
    static ZH: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    match locale {
        LocaleId::ZhCn => ZH.get_or_init(|| parse_catalog(ZH_CN_JSON)),
        LocaleId::EnUs => EN.get_or_init(|| parse_catalog(EN_US_JSON)),
    }
}

pub fn t(locale: LocaleId, key: &str) -> String {
    catalog(locale)
        .get(key)
        .cloned()
        .or_else(|| catalog(LocaleId::EnUs).get(key).cloned())
        .unwrap_or_else(|| key.to_string())
}

fn set_title(app: &AppHandle, label: &str, title: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.set_title(title);
    }
}

pub fn apply_native_ui(app: &AppHandle, locale: LocaleId) {
    set_title(app, "settings", &t(locale, "window.settings"));
    set_title(app, "pairing-show", &t(locale, "window.pairingShow"));
    set_title(app, "pairing-input", &t(locale, "window.pairingInput"));
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(g) = state.inner.tray.lock() {
            if let Some(tray) = g.as_ref() {
                let _ = tray.history.set_text(t(locale, "tray.history"));
                let _ = tray.settings.set_text(t(locale, "tray.settings"));
                let _ = tray.quit.set_text(t(locale, "tray.quit"));
            }
        }
    }
}

pub fn emit_locale_changed(app: &AppHandle, locale: LocaleId) {
    let _ = app.emit("locale-changed", locale.as_str());
}

pub struct TrayMenu {
    pub history: MenuItem<tauri::Wry>,
    pub settings: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_share_keys() {
        let zh = catalog(LocaleId::ZhCn);
        let en = catalog(LocaleId::EnUs);
        assert!(!zh.is_empty());
        assert_eq!(zh.len(), en.len());
        for key in zh.keys() {
            assert!(en.contains_key(key), "missing en-US key {key}");
        }
        for key in en.keys() {
            assert!(zh.contains_key(key), "missing zh-CN key {key}");
        }
    }

    #[test]
    fn resolve_pref() {
        assert_eq!(resolve("en-US"), LocaleId::EnUs);
        assert_eq!(resolve("zh-CN"), LocaleId::ZhCn);
        assert_eq!(normalize_pref("de-DE"), "system");
    }
}
