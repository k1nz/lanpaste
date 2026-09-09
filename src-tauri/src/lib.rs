mod cleanup;
mod clipboard;
mod crypto;
mod device;
mod ipc;
mod net;
mod state;
mod store;
mod types;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, RunEvent};

use crate::state::{AppState, Inner};

pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::list_history,
            ipc::get_entry,
            ipc::paste_entry,
            ipc::hide_overlay,
            ipc::show_overlay,
            ipc::show_settings,
            ipc::copy_entry_to_clipboard,
            ipc::sync_to,
            ipc::delete_entry,
            ipc::reveal_in_finder,
            ipc::list_nearby,
            ipc::list_paired,
            ipc::start_pair,
            ipc::submit_pair_token,
            ipc::cancel_pair,
            ipc::update_device_note,
            ipc::update_device_flags,
            ipc::remove_device,
            ipc::get_settings,
            ipc::update_settings,
            ipc::frontmost_app_name,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let data_dir = handle
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)?;

            let identity = crypto::Identity::load_or_create(&data_dir.join("identity"))?;
            let store = store::Store::open(&data_dir)?;
            let settings = store.load_settings()?;
            let device_name = device::local_device_name();

            let state = AppState {
                inner: Arc::new(Inner {
                    identity,
                    data_dir,
                    store: Mutex::new(store),
                    settings: Mutex::new(settings.clone()),
                    nearby: Mutex::new(HashMap::new()),
                    incoming_pair: Mutex::new(None),
                    outgoing_pair: Mutex::new(None),
                    inflight_blobs: Mutex::new(HashSet::new()),
                    last_change_count: Mutex::new(
                        clipboard::pasteboard_change_count().unwrap_or(0),
                    ),
                    suppress_change_count: Mutex::new(-1),
                    last_frontmost: Mutex::new(None),
                    listen_port: Mutex::new(0),
                    device_name,
                    handle: Mutex::new(Some(handle.clone())),
                    last_pairing_show: Mutex::new(None),
                    last_pairing_input: Mutex::new(None),
                }),
            };
            app.manage(state.clone());

            setup_tray(app)?;
            if let Err(e) = ipc::reregister_shortcut(&handle, &settings.overlay_shortcut) {
                eprintln!("shortcut: {e}");
                let _ = ipc::reregister_shortcut(&handle, types::DEFAULT_SHORTCUT);
            }

            let net_state = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = net::start_network(net_state).await {
                    eprintln!("network: {e}");
                }
            });

            start_clipboard_watcher(state.clone());
            start_cleanup_loop(state);
            Ok(())
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let history = MenuItem::with_id(app, "history", "打开剪贴板历史", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&history, &settings, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("LanPaste")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "history" => ipc::toggle_overlay(app),
            "settings" => {
                let _ = state::show_window(app, "settings");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                ipc::toggle_overlay(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    } else {
        let png = include_bytes!("../icons/32x32.png");
        if let Ok(img) = Image::from_bytes(png) {
            tray = tray.icon(img);
        }
    }
    tray.build(app)?;
    Ok(())
}

fn start_clipboard_watcher(state: AppState) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(400));
        loop {
            tick.tick().await;
            clipboard::track_frontmost();
            let count = match clipboard::pasteboard_change_count() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let last = state
                .inner
                .last_change_count
                .lock()
                .ok()
                .map(|g| *g)
                .unwrap_or(0);
            if count == last {
                continue;
            }
            if let Ok(mut g) = state.inner.last_change_count.lock() {
                *g = count;
            }
            let suppress = state
                .inner
                .suppress_change_count
                .lock()
                .ok()
                .map(|g| *g)
                .unwrap_or(-1);
            if count == suppress {
                continue;
            }
            let captured = match clipboard::read_native() {
                Ok(Some(c)) => c,
                _ => continue,
            };
            let ingested = state.with_store(|s| {
                clipboard::ingest_captured(
                    s,
                    &captured,
                    &state.inner.device_name,
                    &state.inner.identity.instance_id,
                )
            });
            if let Ok(Some(result)) = ingested {
                state.emit_history();
                let id = result.id;
                let st = state.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = net::auto_sync_new_entry(&st, &id).await;
                });
            }
        }
    });
}

fn start_cleanup_loop(state: AppState) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            let settings = match state.inner.settings.lock() {
                Ok(s) => s.clone(),
                Err(_) => continue,
            };
            let inflight = match state.inner.inflight_blobs.lock() {
                Ok(g) => g.clone(),
                Err(_) => continue,
            };
            let _ = state.with_store(|s| cleanup::apply_cleanup(s, &settings, &inflight));
        }
    });
}
