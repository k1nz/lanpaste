pub mod client;
pub mod mdns;
pub mod pair;
pub mod protocol;
pub mod server;
pub mod session;
pub mod sync;

use std::collections::HashSet;

use crate::state::{self, AppState};
use crate::store::now_ms;
use crate::types::{SessionCodeItem, SessionInputPayload, SessionShowPayload};

pub use sync::{auto_sync_new_entry, download_file, flush_pending, sync_entry_to_device};

/// Read/write size for LAN file streams. `ReaderStream::new` defaults to 4 KiB,
/// which turns a 200 MB transfer into tens of thousands of TLS records and UI
/// progress events.
pub const FILE_STREAM_CHUNK: usize = 256 * 1024;

pub async fn start_network(state: AppState) -> Result<u16, String> {
    let identity = state.inner.identity.clone();
    let port = server::serve(state.clone(), identity).await?;
    if let Ok(mut g) = state.inner.listen_port.lock() {
        *g = port;
    }
    let _mdns = mdns::start(state.clone(), port)?;
    let flusher = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            tick.tick().await;
            flush_pending(&flusher).await;
            expire_session_grants(&flusher);
        }
    });
    Ok(port)
}

pub fn refresh_session_show(state: &AppState) {
    let items = state
        .inner
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .pending(now_ms())
        .into_iter()
        .map(|code| SessionCodeItem {
            instance_id: code.instance_id,
            device_name: code.device_name,
            code: code.code,
            expires_at: code.expires_at,
        })
        .collect::<Vec<_>>();
    let Ok(handle) = state.handle() else {
        return;
    };
    if items.is_empty() {
        state::hide_session_show(&handle);
    } else {
        state::show_session_codes(&handle, &SessionShowPayload { items });
    }
}

pub fn expire_session_grants(state: &AppState) {
    if !state.inner.secure_mode {
        return;
    }
    let online = state
        .inner
        .nearby
        .lock()
        .map(|nearby| nearby.keys().cloned().collect::<HashSet<_>>())
        .unwrap_or_default();
    let changed = state
        .inner
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .expire_offline(now_ms(), &online);
    if changed {
        refresh_session_show(state);
    }
}

/// How long a dismissed code window stays quiet before auto-sync may show it
/// again. Short enough that a rotated code is still usable, long enough that
/// the 3s flusher does not steal focus.
const SESSION_PROMPT_SUPPRESS_MS: i64 = 60_000;

/// Open the code entry window for a peer that was refused with `session_locked`.
/// Auto-sync passes `force = false` so an open or just-dismissed window is left
/// alone. A manual send or file paste passes `force = true`.
pub fn note_remote_session_locked(
    state: &AppState,
    instance_id: &str,
    device_name: &str,
    force: bool,
) {
    if !force {
        let input_open = state
            .inner
            .last_session_input
            .lock()
            .ok()
            .is_some_and(|slot| slot.is_some());
        if input_open {
            // Another device is being typed, or this device's window is already up.
            return;
        }
        let now = now_ms();
        let suppressed = state
            .inner
            .session_prompted
            .lock()
            .ok()
            .is_some_and(|prompted| prompted.get(instance_id).is_some_and(|until| now < *until));
        if suppressed {
            return;
        }
    }
    let Ok(handle) = state.handle() else {
        return;
    };
    state::show_session_input(
        &handle,
        &SessionInputPayload {
            instance_id: instance_id.to_string(),
            device_name: device_name.to_string(),
        },
    );
}

pub fn suppress_session_prompt(state: &AppState, instance_id: &str) {
    if let Ok(mut prompted) = state.inner.session_prompted.lock() {
        prompted.insert(
            instance_id.to_string(),
            now_ms().saturating_add(SESSION_PROMPT_SUPPRESS_MS),
        );
    }
}

pub fn clear_session_prompt(state: &AppState, instance_id: &str) {
    if let Ok(mut prompted) = state.inner.session_prompted.lock() {
        prompted.remove(instance_id);
    }
}

pub async fn submit_session_code(
    state: &AppState,
    instance_id: &str,
    code: &str,
) -> Result<(), String> {
    let device = state
        .with_store(|s| s.get_device(instance_id))?
        .ok_or_else(|| "device_removed".to_string())?;
    let nearby = state
        .inner
        .nearby
        .lock()
        .ok()
        .and_then(|n| n.get(instance_id).cloned());
    let host = nearby
        .as_ref()
        .map(|n| n.host.clone())
        .or(device.host.clone())
        .ok_or_else(|| "offline".to_string())?;
    let port = nearby
        .map(|n| n.port)
        .or(device.port)
        .ok_or_else(|| "offline".to_string())?;
    let pin = client::pin_from_device(&device)?;
    let http = client::http_client(Some(pin))?;
    let body = protocol::SessionUnlock {
        code: session::normalize_code(code),
    };
    if body.code.len() != session::CODE_LEN {
        return Err("session_code_invalid".into());
    }
    let bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    let url = client::host_url(&host, port, "/session/unlock");
    let mut req = http.post(url);
    for (k, v) in client::signed_headers(&state.inner.identity, &bytes) {
        req = req.header(k, v);
    }
    let resp = req
        .body(bytes)
        .send()
        .await
        .map_err(|_| "offline".to_string())?;
    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        let t = t.trim();
        return Err(if t.is_empty() {
            "session_code_invalid".into()
        } else {
            t.to_string()
        });
    }
    clear_session_prompt(state, instance_id);
    if let Ok(handle) = state.handle() {
        let close = state
            .inner
            .last_session_input
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|p| p.instance_id == instance_id))
            .unwrap_or(true);
        if close {
            state::hide_session_input(&handle);
        }
    }
    sync::retry_unlocked_sync(state, instance_id).await;
    Ok(())
}

pub async fn start_pair_request(state: &AppState, instance_id: &str) -> Result<(), String> {
    use crate::state::OutgoingPair;
    use crate::types::PairingInputPayload;

    let nearby = state
        .inner
        .nearby
        .lock()
        .map_err(|_| "nearby lock".to_string())?
        .get(instance_id)
        .cloned()
        .ok_or_else(|| "offline".to_string())?;

    let body = protocol::PairRequest {
        instance_id: state.inner.identity.instance_id.clone(),
        name: state.inner.device_name.clone(),
        fingerprint: state.inner.identity.fingerprint.clone(),
        public_key: state.inner.identity.public_key_hex(),
        cert_der: base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &state.inner.identity.cert_der,
        ),
    };
    let client = client::http_client(None)?;
    let url = client::host_url(&nearby.host, nearby.port, "/pair/request");
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|_| "offline".to_string())?;
    if !resp.status().is_success() && resp.status().as_u16() != 204 {
        return Err(format!("pair request failed: {}", resp.status()));
    }
    if let Ok(mut g) = state.inner.outgoing_pair.lock() {
        *g = Some(OutgoingPair {
            instance_id: instance_id.to_string(),
            device_name: nearby.name.clone(),
            host: nearby.host.clone(),
            port: nearby.port,
        });
    }
    let handle = state.handle()?;
    crate::state::show_pairing_input(
        &handle,
        &PairingInputPayload {
            instance_id: instance_id.to_string(),
            device_name: nearby.name,
        },
    );
    Ok(())
}

pub async fn submit_token(state: &AppState, instance_id: &str, token: &str) -> Result<(), String> {
    use crate::store::StoredDevice;

    let outgoing = state
        .inner
        .outgoing_pair
        .lock()
        .map_err(|_| "lock".to_string())?
        .clone()
        .ok_or_else(|| "token_expired".to_string())?;
    if outgoing.instance_id != instance_id {
        return Err("token_invalid".into());
    }
    let nearby = state
        .inner
        .nearby
        .lock()
        .ok()
        .and_then(|n| n.get(instance_id).cloned());
    let host = nearby
        .as_ref()
        .map(|n| n.host.clone())
        .unwrap_or(outgoing.host.clone());
    let port = nearby.map(|n| n.port).unwrap_or(outgoing.port);

    let body = protocol::PairConfirm {
        instance_id: state.inner.identity.instance_id.clone(),
        name: state.inner.device_name.clone(),
        token: token.to_string(),
        fingerprint: state.inner.identity.fingerprint.clone(),
        public_key: state.inner.identity.public_key_hex(),
        cert_der: base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &state.inner.identity.cert_der,
        ),
    };
    let client = client::http_client(None)?;
    let url = client::host_url(&host, port, "/pair/confirm");
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|_| "offline".to_string())?;
    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_else(|_| "token_invalid".into());
        return Err(if t.is_empty() { "token_invalid".into() } else { t });
    }
    let ack: protocol::PairConfirmResponse = resp.json().await.map_err(|e| e.to_string())?;
    let device = StoredDevice {
        instance_id: ack.instance_id,
        name: ack.name,
        note: String::new(),
        fingerprint: ack.fingerprint,
        public_key: ack.public_key,
        cert_der: ack.cert_der,
        allow_send: true,
        allow_receive: true,
        auto_write_clipboard: true,
        trust_broken: false,
        host: Some(host),
        port: Some(port),
    };
    state.with_store(|s| s.upsert_device(&device))?;
    if let Ok(mut g) = state.inner.outgoing_pair.lock() {
        *g = None;
    }
    state.emit_devices();
    if let Ok(h) = state.handle() {
        crate::state::hide_pairing_windows(&h);
    }
    Ok(())
}

/// Best-effort `POST /pair/revoke` to tell a peer to drop us. Returns an error
/// only for the caller to log; the local device is removed regardless.
pub async fn revoke_remote(state: &AppState, instance_id: &str) -> Result<(), String> {
    let device = state
        .with_store(|s| s.get_device(instance_id))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "device_removed".to_string())?;
    let (Some(host), Some(port)) = (device.host.clone(), device.port) else {
        // Never reached from this machine; nothing to revoke remotely.
        return Ok(());
    };
    let pin = client::pin_from_device(&device)?;
    let client = client::http_client(Some(pin))?;
    let body = protocol::PairRevoke {
        instance_id: state.inner.identity.instance_id.clone(),
    };
    // The server verifies these against the raw body bytes.
    let bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    let url = client::host_url(&host, port, "/pair/revoke");
    let mut req = client.post(url);
    for (k, v) in client::signed_headers(&state.inner.identity, &bytes) {
        req = req.header(k, v);
    }
    let resp = req
        .body(bytes)
        .send()
        .await
        .map_err(|_| "offline".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("revoke rejected: {}", resp.status()));
    }
    Ok(())
}
