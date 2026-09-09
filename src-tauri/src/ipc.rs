use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::clipboard;
use crate::device;
use crate::net;
use crate::state::{self, AppState};
use crate::types::{AppSettings, HistoryEntry, NearbyDevice, PairedDevice};

#[tauri::command]
pub fn list_history(
    state: State<AppState>,
    query: Option<String>,
    type_filter: Option<String>,
) -> Result<Vec<HistoryEntry>, String> {
    state.with_store(|s| s.list_history(query.as_deref(), type_filter.as_deref()))
}

#[tauri::command]
pub fn get_entry(state: State<AppState>, id: String) -> Result<Option<HistoryEntry>, String> {
    state.with_store(|s| s.get_entry(&id))
}

#[tauri::command]
pub async fn paste_entry(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let pb = state
        .with_store(|s| s.get_pasteboard(&id))?
        .ok_or_else(|| "条目不存在".to_string())?;
    if pb.needs_file_download {
        net::download_file(&state, &id).await?;
    }
    let payload = state.with_store(|s| clipboard::payload_from_store(s, &id))?;
    let count = clipboard::write_native(&payload)?;
    if let Ok(mut g) = state.inner.suppress_change_count.lock() {
        *g = count;
    }
    let target = state
        .inner
        .last_frontmost
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .filter(|n| !n.is_empty());
    #[cfg(target_os = "windows")]
    {
        let _ = target;
        // Hide first so Windows can restore the previous app, then paste on the UI thread.
        // SetForegroundWindow from a Tauri worker is ignored by the foreground lock.
        let (tx, rx) = std::sync::mpsc::channel();
        let hide_app = app.clone();
        app.run_on_main_thread(move || {
            let _ = state::hide_window(&hide_app, "overlay");
            let _ = tx.send(());
        })
        .map_err(|e| e.to_string())?;
        let _ = rx.recv_timeout(std::time::Duration::from_millis(400));
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(clipboard::activate_app_named(""));
        })
        .map_err(|e| e.to_string())?;
        let _ = rx.recv_timeout(std::time::Duration::from_millis(400));
        // Give the restored edit control time to take focus before Ctrl is held.
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(clipboard::simulate_paste());
        })
        .map_err(|e| e.to_string())?;
        return rx
            .recv_timeout(std::time::Duration::from_millis(800))
            .map_err(|_| "粘贴超时".to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state::hide_window(&app, "overlay");
        if let Some(name) = target {
            let _ = clipboard::activate_app_named(&name);
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        clipboard::simulate_paste()?;
        Ok(())
    }
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Result<(), String> {
    state::hide_window(&app, "overlay")
}

#[tauri::command]
pub fn show_overlay(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    show_overlay_window(&app, Some(&state))
}

#[tauri::command]
pub fn show_settings(app: AppHandle) -> Result<(), String> {
    state::show_window(&app, "settings")
}

#[tauri::command]
pub fn copy_entry_to_clipboard(state: State<AppState>, id: String) -> Result<(), String> {
    let pb = state
        .with_store(|s| s.get_pasteboard(&id))?
        .ok_or_else(|| "条目不存在".to_string())?;
    if pb.needs_file_download {
        return Err("需要先下载文件".into());
    }
    let payload = state.with_store(|s| clipboard::payload_from_store(s, &id))?;
    let count = clipboard::write_native(&payload)?;
    if let Ok(mut g) = state.inner.suppress_change_count.lock() {
        *g = count;
    }
    Ok(())
}

/// `deviceId` is the paired device `instanceId`.
#[tauri::command]
pub async fn sync_to(state: State<'_, AppState>, id: String, device_id: String) -> Result<(), String> {
    net::sync_entry_to_device(&state, &id, &device_id, true).await
}

#[tauri::command]
pub fn delete_entry(state: State<AppState>, id: String) -> Result<(), String> {
    state.with_store(|s| s.delete_pasteboard(&id))?;
    state.emit_history();
    Ok(())
}

#[tauri::command]
pub fn reveal_in_finder(state: State<AppState>, id: String) -> Result<(), String> {
    let path = state.with_store(|s| {
        let items = s.get_items(&id)?;
        if let Some(file) = items.iter().find(|i| i.item_type == "file") {
            if let Some(h) = &file.blob_hash {
                return s.named_blob_path(h, file.file_name.as_deref());
            }
        }
        items
            .iter()
            .find_map(|i| {
                i.blob_hash.as_ref().map(|h| s.blob_path(h)).filter(|p| p.exists())
            })
            .ok_or_else(|| "没有可显示的文件".to_string())
    })?;
    if !path.exists() {
        return Err("没有可显示的文件".into());
    }
    std::process::Command::new("open")
        .args(["-R", &path.to_string_lossy()])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_nearby(state: State<AppState>) -> Result<Vec<NearbyDevice>, String> {
    let nearby = state
        .inner
        .nearby
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone();
    let paired: Vec<String> = state
        .with_store(|s| Ok(s.list_devices()?.into_iter().map(|d| d.instance_id).collect()))?;
    Ok(device::nearby_list(
        &nearby,
        &paired,
        &state.inner.identity.instance_id,
    ))
}

#[tauri::command]
pub fn list_paired(state: State<AppState>) -> Result<Vec<PairedDevice>, String> {
    let devices = state.with_store(|s| s.list_devices())?;
    let nearby = state
        .inner
        .nearby
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone();
    Ok(device::paired_list(&devices, &nearby))
}

#[tauri::command]
pub async fn start_pair(state: State<'_, AppState>, instance_id: String) -> Result<(), String> {
    net::start_pair_request(&state, &instance_id).await
}

#[tauri::command]
pub async fn submit_pair_token(
    state: State<'_, AppState>,
    instance_id: String,
    token: String,
) -> Result<(), String> {
    net::submit_token(&state, &instance_id, &token).await
}

#[tauri::command]
pub fn cancel_pair(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if let Ok(mut g) = state.inner.outgoing_pair.lock() {
        *g = None;
    }
    if let Ok(mut g) = state.inner.incoming_pair.lock() {
        *g = None;
    }
    state::hide_pairing_windows(&app);
    Ok(())
}

#[tauri::command]
pub fn update_device_note(state: State<AppState>, instance_id: String, note: String) -> Result<(), String> {
    state.with_store(|s| s.update_device_note(&instance_id, &note))?;
    state.emit_devices();
    Ok(())
}

#[tauri::command]
pub fn update_device_flags(
    state: State<AppState>,
    instance_id: String,
    allow_send: bool,
    allow_receive: bool,
    auto_write_clipboard: bool,
) -> Result<(), String> {
    state.with_store(|s| {
        s.update_device_flags(&instance_id, allow_send, allow_receive, auto_write_clipboard)
    })?;
    state.emit_devices();
    Ok(())
}

#[tauri::command]
pub async fn remove_device(state: State<'_, AppState>, instance_id: String) -> Result<(), String> {
    net::revoke_remote(&state, &instance_id).await;
    state.with_store(|s| s.remove_device(&instance_id))?;
    state.emit_devices();
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    state
        .inner
        .settings
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "settings lock".into())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<AppState>,
    auto_sync_max_bytes: Option<u64>,
    cleanup_max_items: Option<u64>,
    cleanup_max_bytes: Option<u64>,
    cleanup_max_age_days: Option<Value>,
    overlay_shortcut: Option<String>,
) -> Result<AppSettings, String> {
    let mut settings = state
        .inner
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .clone();
    if let Some(v) = auto_sync_max_bytes {
        settings.auto_sync_max_bytes = v;
    }
    if let Some(v) = cleanup_max_items {
        settings.cleanup_max_items = v;
    }
    if let Some(v) = cleanup_max_bytes {
        settings.cleanup_max_bytes = v;
    }
    if let Some(v) = cleanup_max_age_days {
        settings.cleanup_max_age_days = if v.is_null() {
            None
        } else {
            v.as_u64().map(|n| n as u32)
        };
    }
    if let Some(v) = overlay_shortcut {
        if !v.is_empty() && v != settings.overlay_shortcut {
            reregister_shortcut_replacing(&app, &v, Some(&settings.overlay_shortcut))?;
            settings.overlay_shortcut = v;
        }
    }
    state.with_store(|s| s.save_settings(&settings))?;
    if let Ok(mut g) = state.inner.settings.lock() {
        *g = settings.clone();
    }
    Ok(settings)
}

#[tauri::command]
pub fn frontmost_app_name() -> Result<String, String> {
    clipboard::frontmost_app_name()
}

pub fn show_overlay_window(app: &AppHandle, state: Option<&AppState>) -> Result<(), String> {
    if let Ok(name) = clipboard::remember_frontmost() {
        if let Some(state) = state {
            if let Ok(mut g) = state.inner.last_frontmost.lock() {
                *g = Some(name);
            }
        } else if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut g) = state.inner.last_frontmost.lock() {
                *g = Some(name);
            }
        }
    }
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut g) = state.inner.overlay_blur_hide_at.lock() {
            *g = None;
        }
    }
    state::show_then_emit(app, "overlay", "overlay-shown", ());
    Ok(())
}

/// Hide the overlay shortly after it loses focus (click-away), but ignore
/// transient unfocus that happens while the window is being shown.
pub fn on_overlay_focus_lost(window: &tauri::Window) {
    if window.label() != "overlay" {
        return;
    }
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        if window.is_focused().unwrap_or(false) || !window.is_visible().unwrap_or(false) {
            return;
        }
        let _ = window.hide();
        if let Some(state) = window.app_handle().try_state::<AppState>() {
            if let Ok(mut g) = state.inner.overlay_blur_hide_at.lock() {
                *g = Some(Instant::now());
            }
        }
    });
}

pub fn toggle_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            return;
        }
    }
    // Tray click first unfocuses the overlay; skip the immediate re-show.
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(g) = state.inner.overlay_blur_hide_at.lock() {
            if g.is_some_and(|at| at.elapsed() < Duration::from_millis(280)) {
                return;
            }
        }
    }
    let _ = show_overlay_window(app, None);
}

pub fn reregister_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    reregister_shortcut_replacing(app, shortcut, None)
}

fn bind_overlay_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
    let parsed: Shortcut = shortcut
        .parse()
        .map_err(|e| format!("无效快捷键: {e}"))?;
    app.global_shortcut()
        .on_shortcut(parsed, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_overlay(app);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn reregister_shortcut_replacing(
    app: &AppHandle,
    shortcut: &str,
    previous: Option<&str>,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| format!("无效快捷键: {e}"))?;
    let _ = app.global_shortcut().unregister_all();
    if let Err(e) = bind_overlay_shortcut(app, shortcut) {
        if let Some(prev) = previous {
            let _ = bind_overlay_shortcut(app, prev);
        }
        return Err(e);
    }
    Ok(())
}
