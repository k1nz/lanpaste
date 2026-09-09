use std::collections::HashMap;
use std::io::SeekFrom;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::crypto::{self, Identity};
use crate::device::NearbyInfo;
use crate::state::{AppState, IncomingPair};
use crate::store::{now_ms, StoredDevice, StoredItem, StoredPasteboard};
use crate::types::{
    should_claim_file_promise, PairingShowPayload, PasteType, Preview, PAIR_TOKEN_TTL_MS,
    SYNC_ENTRY_MAX_BODY_BYTES,
};

use super::pair::{token_expires_at, validate_token};
use super::protocol::{
    resolved_content_hash, PairConfirm, PairConfirmResponse, PairRequest, PairRevoke, SyncEntryBody,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/pair/request", post(pair_request))
        .route("/pair/confirm", post(pair_confirm))
        .route("/pair/revoke", post(pair_revoke))
        .route("/files/{id}", get(get_file))
        .merge(
            Router::new()
                .route("/sync/entry", post(sync_entry))
                .layer(DefaultBodyLimit::max(SYNC_ENTRY_MAX_BODY_BYTES)),
        )
        .with_state(state)
}

struct Auth {
    instance_id: String,
}

fn auth_paired(state: &AppState, headers: &HeaderMap, body: &[u8]) -> Result<Auth, (StatusCode, String)> {
    let instance = header_str(headers, "x-lanpaste-instance").ok_or((
        StatusCode::UNAUTHORIZED,
        "device_removed".into(),
    ))?;
    let ts = header_str(headers, "x-lanpaste-ts")
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "device_removed".into()))?;
    let sig = header_str(headers, "x-lanpaste-sig").ok_or((
        StatusCode::UNAUTHORIZED,
        "device_removed".into(),
    ))?;
    let now = now_ms() as u64;
    if ts.abs_diff(now) > 300_000 {
        return Err((StatusCode::UNAUTHORIZED, "device_removed".into()));
    }
    let device = state
        .with_store(|s| s.get_device(&instance))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let Some(device) = device else {
        return Err((StatusCode::UNAUTHORIZED, "device_removed".into()));
    };
    crypto::verify_request(&device.public_key, &instance, ts, body, &sig)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "device_removed".into()))?;
    Ok(Auth {
        instance_id: instance,
    })
}

fn header_str(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

async fn pair_request(
    State(state): State<AppState>,
    Json(body): Json<PairRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let token = crypto::generate_pair_token();
    let expires_at = token_expires_at(now_ms());
    {
        let mut incoming = state
            .inner
            .incoming_pair
            .lock()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lock".into()))?;
        *incoming = Some(IncomingPair {
            peer_instance_id: body.instance_id.clone(),
            peer_name: body.name.clone(),
            peer_fingerprint: body.fingerprint.clone(),
            peer_public_key: body.public_key.clone(),
            peer_cert_der: body.cert_der.clone(),
            token: token.clone(),
            expires_at,
            used: false,
        });
    }
    if let Ok(handle) = state.handle() {
        crate::state::show_pairing_prompt(
            &handle,
            &PairingShowPayload {
                token,
                expires_at,
            },
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn pair_confirm(
    State(state): State<AppState>,
    Json(body): Json<PairConfirm>,
) -> Result<Json<PairConfirmResponse>, (StatusCode, String)> {
    let pending = {
        let mut g = state
            .inner
            .incoming_pair
            .lock()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lock".into()))?;
        let pending = g
            .as_mut()
            .ok_or((StatusCode::BAD_REQUEST, "token_expired".into()))?;
        validate_token(pending, now_ms(), &body.token).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        if pending.peer_instance_id != body.instance_id {
            return Err((StatusCode::BAD_REQUEST, "token_invalid".into()));
        }
        pending.used = true;
        pending.clone()
    };

    let device = StoredDevice {
        instance_id: body.instance_id.clone(),
        name: body.name.clone(),
        note: String::new(),
        fingerprint: body.fingerprint.clone(),
        public_key: body.public_key.clone(),
        cert_der: body.cert_der.clone(),
        allow_send: true,
        allow_receive: true,
        auto_write_clipboard: true,
        trust_broken: false,
        host: None,
        port: None,
    };
    state
        .with_store(|s| s.upsert_device(&device))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    state.emit_devices();

    if let Ok(handle) = state.handle() {
        crate::state::hide_pairing_windows(&handle);
    }

    let _ = pending;
    Ok(Json(PairConfirmResponse {
        instance_id: state.inner.identity.instance_id.clone(),
        name: state.inner.device_name.clone(),
        fingerprint: state.inner.identity.fingerprint.clone(),
        public_key: state.inner.identity.public_key_hex(),
        cert_der: base64::engine::general_purpose::STANDARD.encode(&state.inner.identity.cert_der),
    }))
}

async fn pair_revoke(
    State(state): State<AppState>,
    Json(body): Json<PairRevoke>,
) -> StatusCode {
    let _ = state.with_store(|s| s.remove_device(&body.instance_id));
    state.emit_devices();
    StatusCode::NO_CONTENT
}

async fn sync_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let auth = auth_paired(&state, &headers, &body)?;
    let device = state
        .with_store(|s| s.get_device(&auth.instance_id))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or((StatusCode::UNAUTHORIZED, "device_removed".into()))?;
    if !device.allow_receive {
        return Err((StatusCode::FORBIDDEN, "rejected".into()));
    }

    let parsed: SyncEntryBody =
        serde_json::from_slice(&body).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let exists = state
        .with_store(|s| s.pasteboard_exists(&parsed.id))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if exists {
        return Ok(StatusCode::NO_CONTENT);
    }

    let content_hash = resolved_content_hash(&parsed);
    let echo = state
        .with_store(|s| s.should_skip_duplicate_hash(&content_hash))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if echo {
        return Ok(StatusCode::NO_CONTENT);
    }

    let nearby = state
        .inner
        .nearby
        .lock()
        .ok()
        .and_then(|n| n.get(&auth.instance_id).cloned());
    let (host, port) = nearby
        .map(|n| (Some(n.host), Some(n.port as i64)))
        .unwrap_or((device.host.clone(), device.port.map(|p| p as i64)));

    let inserted = ingest_remote(&state, &parsed, &device, host, port)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    state.emit_history();

    if inserted {
        apply_auto_write(&state, &parsed, &device);
    }

    Ok(StatusCode::NO_CONTENT)
}

fn apply_auto_write(state: &AppState, parsed: &SyncEntryBody, device: &StoredDevice) {
    if !device.auto_write_clipboard {
        return;
    }
    let cap = state
        .inner
        .settings
        .lock()
        .ok()
        .map(|s| s.auto_sync_max_bytes)
        .unwrap_or(crate::types::DEFAULT_AUTO_SYNC_MAX_BYTES);
    let has_file = parsed.items.iter().any(|i| i.item_type == "file");
    if should_claim_file_promise(true, has_file, parsed.total_bytes, cap) {
        match crate::clipboard::write_file_promise(&parsed.id) {
            Ok(count) => state.remember_clipboard_write(count),
            Err(e) => eprintln!("file promise: {e}"),
        }
        return;
    }
    if has_file {
        return;
    }
    let payload = match state.with_store(|s| crate::clipboard::payload_from_store(s, &parsed.id)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("auto-write payload: {e}");
            return;
        }
    };
    if payload.is_empty() {
        return;
    }
    match crate::clipboard::write_native(&payload) {
        Ok(count) => state.remember_clipboard_write(count),
        Err(e) => eprintln!("auto-write: {e}"),
    }
}

fn ingest_remote(
    state: &AppState,
    parsed: &SyncEntryBody,
    device: &StoredDevice,
    host: Option<String>,
    port: Option<i64>,
) -> Result<bool, String> {
    let mut preview = Preview::default();
    let mut items = Vec::new();
    let mut needs_file = false;

    state.with_store(|store| {
        for (idx, it) in parsed.items.iter().enumerate() {
            let mut stored = StoredItem {
                id: it.id.clone(),
                pasteboard_id: parsed.id.clone(),
                sort_order: idx as i64,
                item_type: it.item_type.clone(),
                text_content: it.text.clone(),
                html_content: it.html.clone(),
                rtf_b64: it.rtf_b64.clone(),
                url: it.url.clone(),
                color: it.color.clone(),
                blob_hash: None,
                file_name: it.file_name.clone(),
                file_size: it.file_size,
                width: it.width,
                height: it.height,
                download_token: it.download_token.clone(),
            };
            match it.item_type.as_str() {
                "file" => {
                    needs_file = stored.blob_hash.is_none();
                    preview.file_name = it.file_name.clone();
                    preview.file_size = it.file_size;
                }
                "image" => {
                    if let Some(b64) = &it.image_b64 {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                            let (hash, _) = store.put_blob(&bytes)?;
                            stored.blob_hash = Some(hash.clone());
                            if stored.width.is_none() {
                                if let Some((w, h)) = crate::clipboard::image_dimensions_any(&bytes)
                                {
                                    stored.width = Some(w);
                                    stored.height = Some(h);
                                }
                            }
                            crate::clipboard::apply_image_preview(
                                store, &mut preview, &hash, &bytes,
                            );
                            if preview.width.is_none() {
                                preview.width = it.width;
                                preview.height = it.height;
                            }
                        }
                    }
                }
                "text" => {
                    if !parsed.items.iter().any(|i| i.item_type == "file") {
                        preview.text = it.text.clone();
                    }
                }
                "html" => preview.html = it.html.clone(),
                "url" => preview.url = it.url.clone(),
                "color" => preview.color = it.color.clone(),
                _ => {}
            }
            items.push(stored);
        }

        let pb = StoredPasteboard {
            id: parsed.id.clone(),
            copied_at: parsed.copied_at,
            source_device_id: Some(parsed.source_device_id.clone()),
            source_device_name: parsed.source_device_name.clone(),
            primary_type: parsed.primary_type.clone(),
            title: parsed.title.clone(),
            content_hash: resolved_content_hash(parsed),
            total_bytes: parsed.total_bytes,
            needs_file_download: needs_file,
            file_download_state: "idle".into(),
            download_token: parsed
                .items
                .iter()
                .find_map(|i| i.download_token.clone()),
            source_host: host,
            source_port: port,
            preview_json: serde_json::to_string(&preview).unwrap_or_else(|_| "{}".into()),
        };
        store.insert_pasteboard(&pb, &items)?;
        Ok(())
    })?;
    let _ = device;
    let _ = PasteType::Text;
    Ok(true)
}

async fn get_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    match get_file_inner(&state, &id, &query, &headers).await {
        Ok(r) => r,
        Err((code, msg)) => (code, msg).into_response(),
    }
}

async fn get_file_inner(
    state: &AppState,
    id: &str,
    query: &HashMap<String, String>,
    headers: &HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let body: &[u8] = b"";
    let _auth = auth_paired(state, headers, body)?;
    let token = query.get("token").cloned();

    let item = state
        .with_store(|s| s.find_file_for_download(id))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or((StatusCode::NOT_FOUND, "source_file_gone".into()))?;

    if let (Some(expected), Some(got)) = (item.download_token, token) {
        if expected != got {
            return Err((StatusCode::UNAUTHORIZED, "device_removed".into()));
        }
    }

    let Some(hash) = item.blob_hash else {
        return Err((StatusCode::NOT_FOUND, "source_file_gone".into()));
    };
    let path = state.with_store(|s| Ok(s.blob_path(&hash))).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    })?;
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "source_file_gone".into()));
    }

    if let Ok(mut g) = state.inner.inflight_blobs.lock() {
        g.insert(hash.clone());
    }

    let file_len = item
        .file_size
        .unwrap_or_else(|| std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0));
    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "source_file_gone".into()))?;

    let range = parse_range(headers.get(header::RANGE).and_then(|v| v.to_str().ok()), file_len);
    let (start, end, status) = match range {
        Some((s, e)) => (s, e, StatusCode::PARTIAL_CONTENT),
        None => (0, file_len.saturating_sub(1), StatusCode::OK),
    };
    if start > 0 {
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    let take = end.saturating_sub(start).saturating_add(1);
    let reader = file.take(take);
    let stream = ReaderStream::new(reader);
    let hash_for_drop = hash.clone();
    let state2 = state.clone();
    let stream = stream.map(move |chunk| {
        let _ = &hash_for_drop;
        chunk
    });

    let mut builder = Response::builder().status(status);
    builder = builder.header(header::ACCEPT_RANGES, "bytes");
    builder = builder.header(header::CONTENT_LENGTH, take.to_string());
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{file_len}"),
        );
    }
    let resp = builder
        .body(Body::from_stream(stream))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Drop inflight after a delay; stream drop is racy without pin. Track until task ends.
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if let Ok(mut g) = state2.inner.inflight_blobs.lock() {
            g.remove(&hash);
        }
    });
    Ok(resp)
}

fn parse_range(header: Option<&str>, len: u64) -> Option<(u64, u64)> {
    let h = header?;
    let h = h.strip_prefix("bytes=")?;
    if let Some((a, b)) = h.split_once('-') {
        if a.is_empty() {
            let suffix: u64 = b.parse().ok()?;
            let start = len.saturating_sub(suffix);
            return Some((start, len.saturating_sub(1)));
        }
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() {
            len.saturating_sub(1)
        } else {
            b.parse().ok()?
        };
        if start <= end && end < len {
            return Some((start, end));
        }
    }
    None
}

pub async fn serve(state: AppState, identity: Identity) -> Result<u16, String> {
    use axum_server::tls_rustls::RustlsConfig;

    let listener = std::net::TcpListener::bind("0.0.0.0:0").map_err(|e| format!("bind: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("nonblocking: {e}"))?;
    let port = listener.local_addr().map_err(|e| format!("addr: {e}"))?.port();

    let rustls_config = RustlsConfig::from_der(vec![identity.cert_der.clone()], identity.key_der.clone())
        .await
        .map_err(|e| format!("tls: {e}"))?;
    let app = router(state);

    tokio::spawn(async move {
        if let Err(e) = axum_server::from_tcp_rustls(listener, rustls_config)
            .serve(app.into_make_service())
            .await
        {
            eprintln!("lanpaste https server error: {e}");
        }
    });
    let _ = PAIR_TOKEN_TTL_MS;
    let _ = NearbyInfo {
        instance_id: String::new(),
        name: String::new(),
        fingerprint: String::new(),
        host: String::new(),
        port: 0,
    };
    Ok(port)
}

use futures_util::StreamExt as _;
