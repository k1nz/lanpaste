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

/// Keeps a blob marked in-flight for exactly as long as its response stream may
/// still hand out bytes. Dropped when the stream finishes, when the client
/// disconnects, or when the response is dropped — including on panic — because
/// the guard lives inside the stream.
struct InflightGuard {
    state: AppState,
    hash: String,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let mut g = self
            .state
            .inner
            .inflight_blobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        g.remove(&self.hash);
    }
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
    {
        let mut g = state
            .inner
            .incoming_pair
            .lock()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "lock".into()))?;
        let pending = g
            .as_mut()
            .ok_or((StatusCode::BAD_REQUEST, "token_expired".into()))?;
        validate_token(pending, now_ms(), &body.token).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        // The confirm step must echo exactly what the request step told us to
        // expect; otherwise a caller could pin an arbitrary key/cert.
        validate_pair_confirm(pending, &body).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        pending.used = true;
    }

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

    Ok(Json(PairConfirmResponse {
        instance_id: state.inner.identity.instance_id.clone(),
        name: state.inner.device_name.clone(),
        fingerprint: state.inner.identity.fingerprint.clone(),
        public_key: state.inner.identity.public_key_hex(),
        cert_der: base64::engine::general_purpose::STANDARD.encode(&state.inner.identity.cert_der),
    }))
}

/// Reject a confirm whose identity claims differ from the ones captured when
/// the pairing request arrived, or which are missing/malformed.
fn validate_pair_confirm(pending: &IncomingPair, body: &PairConfirm) -> Result<(), String> {
    if pending.peer_instance_id != body.instance_id
        || pending.peer_name != body.name
        || pending.peer_fingerprint != body.fingerprint
        || pending.peer_public_key != body.public_key
        || pending.peer_cert_der != body.cert_der
    {
        return Err("pair_mismatch".into());
    }
    if body.instance_id.trim().is_empty()
        || body.name.trim().is_empty()
        || body.public_key.trim().is_empty()
        || body.fingerprint.trim().is_empty()
        || body.cert_der.trim().is_empty()
    {
        return Err("pair_mismatch".into());
    }
    let derived = crypto::fingerprint_from_hex_pubkey(&body.public_key)
        .map_err(|_| "pair_mismatch".to_string())?;
    if derived != body.fingerprint {
        return Err("pair_mismatch".into());
    }
    let cert = base64::engine::general_purpose::STANDARD
        .decode(&body.cert_der)
        .map_err(|_| "pair_mismatch".to_string())?;
    if cert.is_empty() {
        return Err("pair_mismatch".into());
    }
    Ok(())
}

async fn pair_revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let auth = auth_paired(&state, &headers, &body)?;
    let parsed: PairRevoke = serde_json::from_slice(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    // A device may only revoke itself: the signed caller id must match the
    // instance id in the body, so no paired peer can delete another device.
    if parsed.instance_id != auth.instance_id {
        return Err((StatusCode::FORBIDDEN, "rejected".into()));
    }
    let _ = state.with_store(|s| s.remove_device(&auth.instance_id));
    state.emit_devices();
    Ok(StatusCode::NO_CONTENT)
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

    // Every downloadable item carries a token, and the caller must present it.
    // (Legacy rows without one are refused rather than left open to any paired
    // device.) A peer fetching a file it was offered sends the token from the
    // sync metadata, so the legitimate download path is unaffected.
    let Some(expected) = item.download_token.as_deref() else {
        return Err((StatusCode::UNAUTHORIZED, "device_removed".into()));
    };
    if token.as_deref() != Some(expected) {
        return Err((StatusCode::UNAUTHORIZED, "device_removed".into()));
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
    // Held by the response stream, so the hash is in-flight for exactly as long
    // as bytes can still be read (stream end, client disconnect, or panic).
    let guard = InflightGuard {
        state: state.clone(),
        hash: hash.clone(),
    };

    let file_len = item
        .file_size
        .unwrap_or_else(|| std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0));
    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "source_file_gone".into()))?;

    let etag = format!("\"{hash}\"");
    // If-Range: a resuming client sends the ETag of the bytes it already holds.
    // A mismatch means the blob changed, so ignore Range and send the whole file.
    let if_range_ok = headers
        .get(header::IF_RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == etag)
        .unwrap_or(true);
    let range = if if_range_ok {
        parse_range(headers.get(header::RANGE).and_then(|v| v.to_str().ok()), file_len)
    } else {
        None
    };
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
    let stream = ReaderStream::new(reader).map(move |chunk| {
        let _in_flight = &guard;
        chunk
    });

    let mut builder = Response::builder().status(status);
    builder = builder.header(header::ACCEPT_RANGES, "bytes");
    builder = builder.header(header::ETAG, etag);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_forms() {
        assert_eq!(parse_range(Some("bytes=0-"), 100), Some((0, 99)));
        assert_eq!(parse_range(Some("bytes=50-99"), 100), Some((50, 99)));
        assert_eq!(parse_range(Some("bytes=50-"), 100), Some((50, 99)));
        assert_eq!(parse_range(Some("bytes=-20"), 100), Some((80, 99)));
        // Open-ended start at EOF is not satisfiable.
        assert_eq!(parse_range(Some("bytes=100-"), 100), None);
        // End past the file is not clamped, matching the existing contract.
        assert_eq!(parse_range(Some("bytes=50-200"), 100), None);
        assert_eq!(parse_range(Some("items=0-10"), 100), None);
        assert_eq!(parse_range(None, 100), None);
    }

    fn pending_for(fp: &str, pk: &str, cert: &str, name: &str, instance: &str) -> IncomingPair {
        IncomingPair {
            peer_instance_id: instance.into(),
            peer_name: name.into(),
            peer_fingerprint: fp.into(),
            peer_public_key: pk.into(),
            peer_cert_der: cert.into(),
            token: "123456".into(),
            expires_at: i64::MAX,
            used: false,
        }
    }

    fn key_material() -> (String, String, String) {
        let sk = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
        let pk = hex::encode(sk.verifying_key().as_bytes());
        let fp = crypto::fingerprint_from_hex_pubkey(&pk).unwrap();
        let cert = base64::engine::general_purpose::STANDARD.encode(b"cert-bytes");
        (pk, fp, cert)
    }

    #[test]
    fn pair_confirm_accepts_matching_identity() {
        let (pk, fp, cert) = key_material();
        let pending = pending_for(&fp, &pk, &cert, "Peer", "dev-1");
        let body = PairConfirm {
            instance_id: "dev-1".into(),
            name: "Peer".into(),
            token: "123456".into(),
            fingerprint: fp,
            public_key: pk,
            cert_der: cert,
        };
        assert!(validate_pair_confirm(&pending, &body).is_ok());
    }

    #[test]
    fn pair_confirm_rejects_changed_identity() {
        let (pk, fp, cert) = key_material();
        let pending = pending_for(&fp, &pk, &cert, "Peer", "dev-1");
        let mut body = PairConfirm {
            instance_id: "dev-1".into(),
            name: "Peer".into(),
            token: "123456".into(),
            fingerprint: fp.clone(),
            public_key: pk.clone(),
            cert_der: cert.clone(),
        };
        body.public_key = hex::encode([9u8; 32]);
        body.fingerprint = crypto::fingerprint_from_hex_pubkey(&body.public_key).unwrap();
        assert_eq!(validate_pair_confirm(&pending, &body).unwrap_err(), "pair_mismatch");
    }

    #[test]
    fn pair_confirm_rejects_malformed_fields() {
        let (pk, fp, cert) = key_material();
        let pending = pending_for(&fp, &pk, &cert, "Peer", "dev-1");

        let mut empty = PairConfirm {
            instance_id: "dev-1".into(),
            name: "".into(),
            token: "123456".into(),
            fingerprint: fp.clone(),
            public_key: pk.clone(),
            cert_der: cert.clone(),
        };
        assert_eq!(validate_pair_confirm(&pending, &empty).unwrap_err(), "pair_mismatch");

        empty.name = "Peer".into();
        empty.public_key = "not-hex".into();
        assert!(validate_pair_confirm(&pending, &empty).is_err());

        let bad_cert = PairConfirm {
            instance_id: "dev-1".into(),
            name: "Peer".into(),
            token: "123456".into(),
            fingerprint: fp,
            public_key: pk,
            cert_der: "!!!not-base64!!!".into(),
        };
        assert_eq!(
            validate_pair_confirm(&pending, &bad_cert).unwrap_err(),
            "pair_mismatch"
        );
    }
}
