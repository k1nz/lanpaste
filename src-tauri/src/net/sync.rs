use std::path::{Path, PathBuf};

use base64::Engine;
use futures_util::StreamExt;
use reqwest::header::{CONTENT_RANGE, ETAG, IF_RANGE, RANGE};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

use crate::state::AppState;
use crate::store::{PendingSync, StoredDevice, StoredItem, StoredPasteboard};
use crate::types::{should_auto_sync, TransferProgressPayload, DEFAULT_AUTO_SYNC_MAX_BYTES};

use super::client::{host_url, http_client, pin_from_device, signed_headers};
use super::protocol::{file_item_has_no_bytes, SyncEntryBody, SyncItem};

/// How many times a retryable queued send is attempted before the row is
/// dropped. At the 3s flusher cadence this bounds a stuck entry's lifetime.
pub const MAX_SYNC_ATTEMPTS: i64 = 5;

/// Errors that are permanent for a given (entry, device) pair: retrying cannot
/// help, so the caller must surface them instead of queueing. Everything else
/// (offline, timeouts, 5xx, transport errors) is retryable.
pub fn is_retryable_sync_error(err: &str) -> bool {
    !matches!(
        err,
        "rejected" | "payload_too_large" | "device_removed" | "trust_broken"
    )
}

/// After a failed attempt (now `attempts_after` attempts), should the queued
/// row stay for another try? Permanent failures are dropped immediately;
/// retryable ones are dropped once the bounded attempt budget is spent.
pub fn queue_after_failure(attempts_after: i64, retryable: bool) -> bool {
    retryable && attempts_after < MAX_SYNC_ATTEMPTS
}

/// Result of comparing an existing partial download against the expected size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeAction {
    /// Start from byte 0 (no usable partial).
    Fresh,
    /// Append starting at this offset.
    Resume(u64),
    /// The partial already holds the whole file; finalize without the network.
    AlreadyComplete(u64),
}

/// Decide how to resume from a partial of `partial_len` bytes when the total
/// size is known (or not). A partial longer than the expected total is treated
/// as corrupt and restarted.
pub fn plan_resume(partial_len: u64, expected_total: Option<u64>) -> ResumeAction {
    if partial_len == 0 {
        return ResumeAction::Fresh;
    }
    if let Some(total) = expected_total {
        if partial_len == total {
            return ResumeAction::AlreadyComplete(partial_len);
        }
        if partial_len > total {
            return ResumeAction::Fresh;
        }
    }
    ResumeAction::Resume(partial_len)
}

/// Parse the total length out of a `Content-Range: bytes a-b/total` header.
pub fn parse_content_range_total(header: Option<&str>) -> Option<u64> {
    let (_, total) = header?.rsplit_once('/')?;
    let total = total.trim();
    if total == "*" {
        return None;
    }
    total.parse().ok()
}

pub fn build_sync_body(
    pb_id: &str,
    copied_at: i64,
    source_device_id: &str,
    source_device_name: &str,
    primary_type: &str,
    title: &str,
    total_bytes: u64,
    content_hash: &str,
    items: &[StoredItem],
    image_bytes: &[(String, Vec<u8>)],
) -> SyncEntryBody {
    let has_file = items.iter().any(|it| it.item_type == "file");
    let items = items
        .iter()
        .map(|it| {
            let mut si = SyncItem {
                id: it.id.clone(),
                item_type: it.item_type.clone(),
                text: it.text_content.clone(),
                html: it.html_content.clone(),
                rtf_b64: it.rtf_b64.clone(),
                url: it.url.clone(),
                color: it.color.clone(),
                image_b64: None,
                file_name: it.file_name.clone(),
                file_size: it.file_size,
                file_hash: it.blob_hash.clone(),
                download_token: it.download_token.clone(),
                width: it.width,
                height: it.height,
            };
            // Files: metadata only. Finder/Explorer also put a TIFF/PNG preview on
            // the same pasteboard — never inline that, or POST /sync/entry blows
            // past the receiver body limit.
            if it.item_type == "image" && !has_file {
                if let Some((_, bytes)) = image_bytes.iter().find(|(id, _)| id == &it.id) {
                    si.image_b64 = Some(base64::engine::general_purpose::STANDARD.encode(bytes));
                }
            }
            if it.item_type == "file" {
                si.image_b64 = None;
            }
            si
        })
        .collect();
    SyncEntryBody {
        id: pb_id.to_string(),
        copied_at,
        source_device_id: source_device_id.to_string(),
        source_device_name: source_device_name.to_string(),
        primary_type: primary_type.to_string(),
        title: title.to_string(),
        total_bytes,
        content_hash: content_hash.to_string(),
        items,
    }
}

pub async fn post_sync(state: &AppState, device: &StoredDevice, body: &SyncEntryBody) -> Result<(), String> {
    let host = device
        .host
        .clone()
        .ok_or_else(|| "offline".to_string())?;
    let port = device.port.ok_or_else(|| "offline".to_string())?;
    let pin = pin_from_device(device)?;
    let client = http_client(Some(pin)).map_err(|e| {
        if e.contains("trust") {
            "trust_broken".into()
        } else {
            e
        }
    })?;
    let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
    let mut req = client.post(host_url(&host, port, "/sync/entry"));
    for (k, v) in signed_headers(&state.inner.identity, &bytes) {
        req = req.header(k, v);
    }
    let resp = req.body(bytes).send().await.map_err(|e| map_net_err(e))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err("device_removed".into());
    }
    if status.as_u16() == 403 {
        return Err("rejected".into());
    }
    if status.as_u16() == 413 {
        return Err("payload_too_large".into());
    }
    if status.is_server_error() {
        // Transient: the peer is reachable but could not accept the entry now.
        return Err("offline".into());
    }
    if !status.is_success() {
        let t = resp.text().await.unwrap_or_default();
        if t.contains("length limit exceeded") {
            return Err("payload_too_large".into());
        }
        // Any other 4xx is a permanent rejection for this entry.
        return Err("rejected".into());
    }
    Ok(())
}

fn map_net_err(e: reqwest::Error) -> String {
    let s = e.to_string();
    if s.contains("trust_broken") || s.contains("certificate") {
        "trust_broken".into()
    } else if e.is_connect() || e.is_timeout() {
        "offline".into()
    } else {
        s
    }
}

pub async fn sync_entry_to_device(
    state: &AppState,
    pasteboard_id: &str,
    device_id: &str,
    ignore_cap: bool,
) -> Result<(), String> {
    let settings = state
        .inner
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .clone();
    let (pb, items, device) = state.with_store(|s| {
        let pb = s
            .get_pasteboard(pasteboard_id)?
            .ok_or_else(|| "条目不存在".to_string())?;
        let items = s.get_items(pasteboard_id)?;
        let device = s
            .get_device(device_id)?
            .ok_or_else(|| "device_removed".to_string())?;
        Ok((pb, items, device))
    })?;
    if !device.allow_send {
        return Err("rejected".into());
    }
    if !ignore_cap && !should_auto_sync(pb.total_bytes, settings.auto_sync_max_bytes) {
        return Ok(());
    }
    let nearby = state
        .inner
        .nearby
        .lock()
        .ok()
        .and_then(|n| n.get(device_id).cloned());
    let device = if let Some(n) = nearby {
        let mut d = device;
        d.host = Some(n.host);
        d.port = Some(n.port);
        d
    } else if device.host.is_none() {
        return Err("offline".into());
    } else {
        device
    };

    let has_file = items.iter().any(|it| it.item_type == "file");
    let mut image_bytes = Vec::new();
    if !has_file {
        for it in &items {
            if it.item_type == "image" {
                if let Some(hash) = &it.blob_hash {
                    if let Ok(bytes) = state.with_store(|s| s.read_blob(hash)) {
                        image_bytes.push((it.id.clone(), bytes));
                    }
                }
            }
        }
    }

    let body = build_sync_body(
        &pb.id,
        pb.copied_at,
        pb.source_device_id
            .as_deref()
            .unwrap_or(&state.inner.identity.instance_id),
        &pb.source_device_name,
        &pb.primary_type,
        &pb.title,
        pb.total_bytes,
        &pb.content_hash,
        &items,
        &image_bytes,
    );
    debug_assert!(body.items.iter().filter(|i| i.item_type == "file").all(file_item_has_no_bytes));

    match post_sync(state, &device, &body).await {
        Ok(()) => {
            let _ = state.with_store(|s| s.dequeue_sync(pasteboard_id, device_id));
            Ok(())
        }
        Err(e) => {
            if e == "trust_broken" {
                let _ = state.with_store(|s| s.mark_trust_broken(device_id));
                state.emit_devices();
            }
            Err(e)
        }
    }
}

pub async fn auto_sync_new_entry(state: &AppState, pasteboard_id: &str) -> Result<(), String> {
    let settings = state
        .inner
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .clone();
    let total = state.with_store(|s| {
        Ok(s.get_pasteboard(pasteboard_id)?
            .map(|p| p.total_bytes)
            .unwrap_or(u64::MAX))
    })?;
    if !should_auto_sync(total, settings.auto_sync_max_bytes) {
        return Ok(());
    }
    let devices = state.with_store(|s| s.list_devices())?;
    let nearby = state
        .inner
        .nearby
        .lock()
        .map_err(|_| "nearby lock".to_string())?
        .clone();
    let mut first_permanent: Option<String> = None;
    for d in devices {
        if !d.allow_send || d.trust_broken {
            continue;
        }
        let online = nearby.contains_key(&d.instance_id);
        if !online {
            // Offline peers are queued on disk and retried by the flusher.
            let _ = state.with_store(|s| s.enqueue_sync(pasteboard_id, &d.instance_id));
            continue;
        }
        match sync_entry_to_device(state, pasteboard_id, &d.instance_id, false).await {
            Ok(()) => {}
            Err(e) if is_retryable_sync_error(&e) => {
                // Reachable but the send did not land: keep it for retry instead
                // of dropping it on the floor.
                let _ = state.with_store(|s| s.enqueue_sync(pasteboard_id, &d.instance_id));
            }
            Err(e) => {
                // Permanent (rejected / too large / removed / trust broken):
                // never retry, and do not swallow it silently.
                if first_permanent.is_none() {
                    first_permanent = Some(e);
                }
            }
        }
    }
    match first_permanent {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn drop_pending(state: &AppState, entry: &PendingSync, reason: &str) {
    let _ = state.with_store(|s| s.dequeue_sync(&entry.pasteboard_id, &entry.device_id));
    eprintln!(
        "lanpaste pending sync dropped: pasteboard={} device={} reason={reason} last_error={:?}",
        entry.pasteboard_id, entry.device_id, entry.last_error
    );
    if let Ok(h) = state.handle() {
        let _ = h.emit(
            "sync-queue-dropped",
            serde_json::json!({
                "pasteboardId": entry.pasteboard_id,
                "deviceId": entry.device_id,
                "reason": reason,
                "lastError": entry.last_error,
            }),
        );
    }
}

pub async fn flush_pending(state: &AppState) {
    let pending = match state.with_store(|s| s.list_pending_sync()) {
        Ok(p) => p,
        Err(_) => return,
    };
    let nearby = match state.inner.nearby.lock() {
        Ok(g) => g.clone(),
        Err(_) => return,
    };
    let cap = state
        .inner
        .settings
        .lock()
        .map(|s| s.auto_sync_max_bytes)
        .unwrap_or(DEFAULT_AUTO_SYNC_MAX_BYTES);
    for entry in pending {
        if !nearby.contains_key(&entry.device_id) {
            continue;
        }
        let pb = match state.with_store(|s| s.get_pasteboard(&entry.pasteboard_id)) {
            Ok(Some(pb)) => pb,
            Ok(None) => {
                drop_pending(state, &entry, "item_removed");
                continue;
            }
            Err(_) => continue,
        };
        // The cap may have been lowered since the row was queued. That is a
        // permanent condition for this entry, so evict it rather than retrying
        // it every tick forever.
        if !should_auto_sync(pb.total_bytes, cap) {
            drop_pending(state, &entry, "payload_too_large");
            continue;
        }
        match sync_entry_to_device(state, &entry.pasteboard_id, &entry.device_id, false).await {
            Ok(()) => {}
            Err(e) if is_retryable_sync_error(&e) => {
                let attempts = state
                    .with_store(|s| {
                        s.bump_pending_sync(&entry.pasteboard_id, &entry.device_id, &e)
                    })
                    .unwrap_or(entry.attempts + 1);
                if !queue_after_failure(attempts, true) {
                    drop_pending(state, &entry, &e);
                }
            }
            Err(e) => drop_pending(state, &entry, &e),
        }
    }
}

pub async fn download_file(state: &AppState, pasteboard_id: &str) -> Result<(), String> {
    // Two overlapping downloads of the same file must not interleave writes
    // into the same partial file.
    let lock = state.download_lock(pasteboard_id);
    let _serialized = lock.lock().await;

    let (pb, items) = state.with_store(|s| {
        let pb = s
            .get_pasteboard(pasteboard_id)?
            .ok_or_else(|| "条目不存在".to_string())?;
        let items = s.get_items(pasteboard_id)?;
        Ok((pb, items))
    })?;
    let file = items
        .iter()
        .find(|i| i.item_type == "file")
        .ok_or_else(|| "不是文件条目".to_string())?;
    if file.blob_hash.as_ref().is_some_and(|h| state.with_store(|s| Ok(s.blob_path(h).exists())).unwrap_or(false)) {
        let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "idle", false));
        return Ok(());
    }

    let source_id = pb
        .source_device_id
        .clone()
        .ok_or_else(|| "source_file_gone".to_string())?;
    let mut device = state
        .with_store(|s| s.get_device(&source_id))?
        .ok_or_else(|| "offline".to_string())?;
    if let Ok(n) = state.inner.nearby.lock() {
        if let Some(info) = n.get(&source_id) {
            device.host = Some(info.host.clone());
            device.port = Some(info.port);
        }
    }
    let host = device.host.clone().or(pb.source_host.clone()).ok_or_else(|| "offline".to_string())?;
    let port = device.port.or(pb.source_port.map(|p| p as u16)).ok_or_else(|| "offline".to_string())?;
    let token = file
        .download_token
        .clone()
        .or(pb.download_token.clone())
        .unwrap_or_default();
    let file_id = file.id.clone();
    let expected_size = file.file_size;

    let partial = state.with_store(|s| Ok(s.partial_download_path(&file_id)))?;
    if let Some(parent) = partial.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("partial dir: {e}"))?;
    }
    let existing = std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
    let resume_from = match plan_resume(existing, expected_size) {
        ResumeAction::Fresh => 0,
        ResumeAction::Resume(off) => off,
        ResumeAction::AlreadyComplete(off) => {
            // The process died after the bytes arrived but before they were
            // committed; finish the move instead of downloading again.
            let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "downloading", true));
            state.emit_history();
            match finalize_download(state, pasteboard_id, file, &pb, &partial, Some(off)) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    discard_partial(&partial);
                    fail_download(state, pasteboard_id);
                    return Err(e);
                }
            }
        }
    };

    let pin = pin_from_device(&device)?;
    let client = http_client(Some(pin))?;
    let url = format!(
        "{}?token={}",
        host_url(&host, port, &format!("/files/{file_id}")),
        urlencoding(&token)
    );
    let mut req = client.get(url);
    for (k, v) in signed_headers(&state.inner.identity, b"") {
        req = req.header(k, v);
    }
    if resume_from > 0 {
        req = req.header(RANGE, format!("bytes={resume_from}-"));
        // Only append to bytes we know came from the same blob version.
        if let Some(validator) = read_validator(&partial) {
            req = req.header(IF_RANGE, validator);
        }
    }

    let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "downloading", true));
    state.emit_history();

    let resp = req.send().await.map_err(|e| {
        fail_download(state, pasteboard_id);
        map_net_err(e)
    })?;
    let status = resp.status();
    if !status.is_success() {
        let code = status.as_u16();
        let server_error = status.is_server_error();
        let t = resp.text().await.unwrap_or_default();
        let err = if code == 401 || t.contains("device_removed") {
            "device_removed"
        } else if code == 403 || t.contains("rejected") {
            "rejected"
        } else if code == 416 {
            // Stale/oversized partial: drop it and let the next attempt restart.
            "offline"
        } else if server_error {
            // Transient: keep the partial so the next attempt can resume.
            "offline"
        } else {
            "source_file_gone"
        };
        if !server_error {
            discard_partial(&partial);
        }
        fail_download(state, pasteboard_id);
        return Err(err.into());
    }

    // `received` is cumulative across attempts so the progress bar never jumps
    // backwards on resume.
    let mut received = resume_from;
    let mut total = expected_size.unwrap_or(0);
    if status.as_u16() == 206 {
        if let Some(t) =
            parse_content_range_total(resp.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()))
        {
            total = t;
        } else if let Some(cl) = resp.content_length() {
            total = received.saturating_add(cl);
        }
        if let Some(etag) = resp.headers().get(ETAG).and_then(|v| v.to_str().ok()) {
            write_validator(&partial, etag);
        }
    } else {
        // A 200 answer to a ranged request means the server would not honour
        // the offset (changed blob or no Range support): restart from zero so
        // stale bytes are never spliced onto a different file.
        received = 0;
        total = resp.content_length().or(expected_size).unwrap_or(0);
        if resume_from > 0 {
            discard_partial(&partial);
        }
        // Remember the validator so an interrupted first attempt can If-Range
        // on its retry.
        if let Some(etag) = resp.headers().get(ETAG).and_then(|v| v.to_str().ok()) {
            write_validator(&partial, etag);
        }
    }

    let mut out = if received == 0 {
        tokio::fs::File::create(&partial).await
    } else {
        tokio::fs::OpenOptions::new().append(true).open(&partial).await
    }
    .map_err(|e| {
        fail_download(state, pasteboard_id);
        format!("partial file: {e}")
    })?;

    let handle = state.handle().ok();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            // Keep whatever landed on disk; the next attempt resumes from there.
            fail_download(state, pasteboard_id);
            map_net_err(e)
        })?;
        if let Err(e) = out.write_all(&chunk).await {
            fail_download(state, pasteboard_id);
            return Err(format!("partial write: {e}"));
        }
        received += chunk.len() as u64;
        if let Some(h) = &handle {
            let _ = h.emit(
                "transfer-progress",
                TransferProgressPayload {
                    id: pasteboard_id.to_string(),
                    received,
                    total,
                },
            );
        }
    }
    let _ = out.flush().await;
    drop(out);

    if total > 0 && received != total {
        fail_download(state, pasteboard_id);
        return Err("offline".into());
    }

    let expected_total = if total > 0 { Some(total) } else { expected_size };
    match finalize_download(state, pasteboard_id, file, &pb, &partial, expected_total) {
        Ok(()) => Ok(()),
        Err(e) => {
            discard_partial(&partial);
            fail_download(state, pasteboard_id);
            Err(e)
        }
    }
}

fn fail_download(state: &AppState, pasteboard_id: &str) {
    let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "failed", true));
    state.emit_history();
}

/// Turn a fully-downloaded partial into the content-addressed blob and attach
/// it to the item. Called with the partial still on disk.
fn finalize_download(
    state: &AppState,
    pasteboard_id: &str,
    file: &StoredItem,
    pb: &StoredPasteboard,
    partial: &Path,
    expected_total: Option<u64>,
) -> Result<(), String> {
    let magic = read_prefix(partial, 64);
    let (hash, size) = state.with_store(|s| s.commit_download(partial, expected_total))?;
    state.with_store(|s| {
        let mut preview: crate::types::Preview =
            serde_json::from_str(&pb.preview_json).unwrap_or_default();
        let dest = s
            .named_blob_path(&hash, file.file_name.as_deref())
            .unwrap_or_else(|_| s.blob_path(&hash));
        preview.path = Some(dest.to_string_lossy().into_owned());
        preview.file_size = Some(size);
        if crate::clipboard::looks_like_image_name(file.file_name.as_deref().unwrap_or(""))
            || crate::clipboard::looks_like_image_magic(&magic)
        {
            crate::clipboard::apply_image_thumb_from_path(&mut preview, &dest);
        }
        let preview_json = serde_json::to_string(&preview).unwrap_or_else(|_| "{}".into());
        s.attach_blob_to_file_item(pasteboard_id, &hash, size, &preview_json)?;
        Ok(())
    })?;
    state.emit_history();
    Ok(())
}

fn validator_path(partial: &Path) -> PathBuf {
    partial.with_extension("etag")
}

fn read_validator(partial: &Path) -> Option<String> {
    std::fs::read_to_string(validator_path(partial))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_validator(partial: &Path, validator: &str) {
    let _ = std::fs::write(validator_path(partial), validator);
}

fn discard_partial(partial: &Path) {
    let _ = std::fs::remove_file(partial);
    let _ = std::fs::remove_file(validator_path(partial));
}

fn read_prefix(path: &Path, n: usize) -> Vec<u8> {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut buf = vec![0u8; n];
    match file.read(&mut buf) {
        Ok(read) => {
            buf.truncate(read);
            buf
        }
        Err(_) => Vec::new(),
    }
}

fn urlencoding(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StoredItem;

    #[test]
    fn auto_sync_file_payload_has_no_bytes() {
        let items = vec![StoredItem {
            id: "i1".into(),
            pasteboard_id: "p1".into(),
            sort_order: 0,
            item_type: "file".into(),
            text_content: None,
            html_content: None,
            rtf_b64: None,
            url: None,
            color: None,
            blob_hash: Some("abc".into()),
            file_name: Some("a.bin".into()),
            file_size: Some(1024),
            width: None,
            height: None,
            download_token: Some("tok".into()),
        }];
        let fake_bytes = vec![("i1".into(), vec![1, 2, 3, 4])];
        let body = build_sync_body(
            "p1",
            1,
            "dev",
            "Dev",
            "file",
            "a.bin",
            1024,
            "hash-file",
            &items,
            &fake_bytes,
        );
        assert_eq!(body.items.len(), 1);
        assert!(file_item_has_no_bytes(&body.items[0]));
        assert!(body.items[0].image_b64.is_none());
        assert_eq!(body.items[0].file_name.as_deref(), Some("a.bin"));
        assert_eq!(body.items[0].file_hash.as_deref(), Some("abc"));
        assert_eq!(body.items[0].download_token.as_deref(), Some("tok"));
    }

    fn item(id: &str, ty: &str) -> StoredItem {
        StoredItem {
            id: id.into(),
            pasteboard_id: "p1".into(),
            sort_order: 0,
            item_type: ty.into(),
            text_content: None,
            html_content: None,
            rtf_b64: None,
            url: None,
            color: None,
            blob_hash: Some("hash".into()),
            file_name: if ty == "file" {
                Some("photo.png".into())
            } else {
                None
            },
            file_size: Some(3 * 1024 * 1024),
            width: None,
            height: None,
            download_token: if ty == "file" {
                Some("tok".into())
            } else {
                None
            },
        }
    }

    #[test]
    fn file_copy_omits_sidecar_image_bytes() {
        let preview = vec![0u8; 3 * 1024 * 1024];
        let body = build_sync_body(
            "p1",
            1,
            "dev",
            "Dev",
            "file",
            "photo.png",
            preview.len() as u64,
            "hash-file-img",
            &[item("f1", "file"), item("img", "image")],
            &[("img".into(), preview)],
        );
        let image = body
            .items
            .iter()
            .find(|i| i.item_type == "image")
            .expect("image item");
        assert!(image.image_b64.is_none());
        assert!(body
            .items
            .iter()
            .filter(|i| i.item_type == "file")
            .all(file_item_has_no_bytes));
    }

    #[test]
    fn image_only_entry_includes_bytes() {
        let png = vec![0x89, 0x50, 0x4E, 0x47, 1, 2, 3];
        let body = build_sync_body(
            "p1",
            1,
            "dev",
            "Dev",
            "image",
            "图片",
            png.len() as u64,
            "hash-img",
            &[item("img", "image")],
            &[("img".into(), png.clone())],
        );
        assert_eq!(body.content_hash, "hash-img");
        let b64 = body.items[0].image_b64.as_deref().expect("image_b64");
        assert_eq!(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).unwrap(),
            png
        );
    }

    #[test]
    fn resume_plan_start_append_and_restart() {
        assert_eq!(plan_resume(0, Some(100)), ResumeAction::Fresh);
        assert_eq!(plan_resume(0, None), ResumeAction::Fresh);
        assert_eq!(plan_resume(30, Some(100)), ResumeAction::Resume(30));
        assert_eq!(plan_resume(30, None), ResumeAction::Resume(30));
        // A partial that already holds the whole file is finalized, not re-fetched.
        assert_eq!(plan_resume(100, Some(100)), ResumeAction::AlreadyComplete(100));
        // Over-long partial (corrupt / different blob) restarts.
        assert_eq!(plan_resume(120, Some(100)), ResumeAction::Fresh);
    }

    #[test]
    fn content_range_total_parsing() {
        assert_eq!(
            parse_content_range_total(Some("bytes 100-199/1000")),
            Some(1000)
        );
        assert_eq!(parse_content_range_total(Some("bytes 0-0/*")), None);
        assert_eq!(parse_content_range_total(Some("nonsense")), None);
        assert_eq!(parse_content_range_total(None), None);
    }

    #[test]
    fn retryable_errors_are_classified_explicitly() {
        for permanent in ["rejected", "payload_too_large", "device_removed", "trust_broken"] {
            assert!(!is_retryable_sync_error(permanent), "{permanent}");
        }
        for transient in ["offline", "timeout", "connection reset"] {
            assert!(is_retryable_sync_error(transient), "{transient}");
        }
    }

    #[test]
    fn queue_evicts_after_bounded_attempts() {
        assert!(queue_after_failure(1, true));
        assert!(queue_after_failure(MAX_SYNC_ATTEMPTS - 1, true));
        assert!(!queue_after_failure(MAX_SYNC_ATTEMPTS, true));
        // Permanent failures never stay, regardless of attempt count.
        assert!(!queue_after_failure(1, false));
    }
}
