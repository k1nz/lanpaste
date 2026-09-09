use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::crypto::Identity;
use crate::device::NearbyInfo;
use crate::i18n::TrayMenu;
use crate::store::Store;
use crate::types::{AppSettings, PairingInputPayload, PairingShowPayload};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const GLASS_WINDOWS: &[&str] = &["overlay", "pairing-show", "pairing-input"];
#[cfg(target_os = "macos")]
const GLASS_RADIUS: f64 = 12.0;
#[cfg(target_os = "windows")]
const GLASS_TINT: (u8, u8, u8, u8) = (28, 28, 30, 160);

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Inner>,
}

pub struct Inner {
    pub identity: Identity,
    pub data_dir: PathBuf,
    pub store: Mutex<Store>,
    pub settings: Mutex<AppSettings>,
    pub nearby: Mutex<HashMap<String, NearbyInfo>>,
    pub incoming_pair: Mutex<Option<IncomingPair>>,
    pub outgoing_pair: Mutex<Option<OutgoingPair>>,
    pub inflight_blobs: Mutex<HashSet<String>>,
    pub last_change_count: Mutex<i64>,
    pub suppress_change_count: Mutex<i64>,
    pub last_frontmost: Mutex<Option<String>>,
    pub listen_port: Mutex<u16>,
    pub device_name: String,
    pub handle: Mutex<Option<AppHandle>>,
    pub last_pairing_show: Mutex<Option<PairingShowPayload>>,
    pub last_pairing_input: Mutex<Option<PairingInputPayload>>,
    pub overlay_blur_hide_at: Mutex<Option<Instant>>,
    pub tray: Mutex<Option<TrayMenu>>,
}

#[derive(Debug, Clone)]
pub struct IncomingPair {
    pub peer_instance_id: String,
    pub peer_name: String,
    pub peer_fingerprint: String,
    pub peer_public_key: String,
    pub peer_cert_der: String,
    pub token: String,
    pub expires_at: i64,
    pub used: bool,
}

#[derive(Debug, Clone)]
pub struct OutgoingPair {
    pub instance_id: String,
    pub device_name: String,
    pub host: String,
    pub port: u16,
}

impl AppState {
    pub fn handle(&self) -> Result<AppHandle, String> {
        self.inner
            .handle
            .lock()
            .map_err(|_| "lock".to_string())?
            .clone()
            .ok_or_else(|| "app handle missing".into())
    }

    pub fn emit_history(&self) {
        if let Ok(h) = self.handle() {
            let _ = h.emit("history-changed", ());
        }
    }

    pub fn emit_devices(&self) {
        if let Ok(h) = self.handle() {
            let _ = h.emit("devices-changed", ());
        }
    }

    pub fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
        let mut store = self
            .inner
            .store
            .lock()
            .map_err(|_| "store lock poisoned".to_string())?;
        f(&mut store)
    }
}

pub fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let w = app
        .get_webview_window(label)
        .ok_or_else(|| format!("window {label} missing"))?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if GLASS_WINDOWS.contains(&w.label()) {
        let window = w.clone();
        window
            .run_on_main_thread({
                let window = window.clone();
                move || {
                    apply_glass_effect(&window);
                    let _ = window.show();
                    // DWM/SWCA acrylic often only composites after the HWND is visible.
                    #[cfg(target_os = "windows")]
                    apply_glass_effect(&window);
                    let _ = window.set_focus();
                }
            })
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn apply_glass_effect(window: &tauri::WebviewWindow) {
    let _ = window.set_theme(Some(tauri::Theme::Dark));
    #[cfg(target_os = "macos")]
    {
        let _ = window_vibrancy::clear_vibrancy(window);
        let _ = window_vibrancy::apply_vibrancy(
            window,
            window_vibrancy::NSVisualEffectMaterial::HudWindow,
            Some(window_vibrancy::NSVisualEffectState::Active),
            Some(GLASS_RADIUS),
        );
    }
    #[cfg(target_os = "windows")]
    {
        let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
        let tint = Some(GLASS_TINT);
        let _ = window_vibrancy::apply_acrylic(window, tint);
        let _ = window_vibrancy::apply_blur(window, tint);
    }
}

pub fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(label) {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Show first, then emit so Vue `onMounted` listeners are attached. Re-emit shortly after.
pub fn show_then_emit<T>(app: &AppHandle, label: &str, event: &'static str, payload: T)
where
    T: Serialize + Clone + Send + 'static,
{
    let _ = show_window(app, label);
    let _ = app.emit(event, payload.clone());
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let _ = app2.emit(event, payload);
    });
}

pub fn hide_pairing_windows(app: &AppHandle) {
    let _ = hide_window(app, "pairing-show");
    let _ = hide_window(app, "pairing-input");
    let _ = app.emit("pairing-hide", ());
}

pub fn show_pairing_prompt(app: &AppHandle, payload: &PairingShowPayload) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut g) = state.inner.last_pairing_show.lock() {
            *g = Some(payload.clone());
        }
    }
    show_then_emit(app, "pairing-show", "pairing-show", payload.clone());
}

pub fn show_pairing_input(app: &AppHandle, payload: &PairingInputPayload) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut g) = state.inner.last_pairing_input.lock() {
            *g = Some(payload.clone());
        }
    }
    show_then_emit(app, "pairing-input", "pairing-input", payload.clone());
}
