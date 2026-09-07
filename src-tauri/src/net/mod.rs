pub mod client;
pub mod mdns;
pub mod pair;
pub mod protocol;
pub mod server;
pub mod sync;

use crate::state::AppState;

pub use sync::{auto_sync_new_entry, download_file, flush_pending, sync_entry_to_device};

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
        }
    });
    Ok(port)
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

pub async fn revoke_remote(state: &AppState, instance_id: &str) {
    let device = state.with_store(|s| s.get_device(instance_id)).ok().flatten();
    let Some(device) = device else {
        return;
    };
    let (Some(host), Some(port)) = (device.host.clone(), device.port) else {
        return;
    };
    let pin = client::pin_from_device(&device).ok();
    let client = match client::http_client(pin) {
        Ok(c) => c,
        Err(_) => return,
    };
    let body = protocol::PairRevoke {
        instance_id: state.inner.identity.instance_id.clone(),
    };
    let url = client::host_url(&host, port, "/pair/revoke");
    let _ = client.post(url).json(&body).send().await;
}
