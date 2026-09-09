use base64::Engine;
use futures_util::StreamExt;
use tauri::Emitter;

use crate::state::AppState;
use crate::store::{StoredDevice, StoredItem};
use crate::types::{should_auto_sync, TransferProgressPayload};

use super::client::{host_url, http_client, pin_from_device, signed_headers};
use super::protocol::{file_item_has_no_bytes, SyncEntryBody, SyncItem};

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
    if !status.is_success() {
        let t = resp.text().await.unwrap_or_default();
        if t.contains("length limit exceeded") {
            return Err("payload_too_large".into());
        }
        return Err(if t.is_empty() {
            format!("sync failed: {status}")
        } else {
            t
        });
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
    for d in devices {
        if !d.allow_send || d.trust_broken {
            continue;
        }
        let online = nearby.contains_key(&d.instance_id);
        if online {
            if let Err(e) = sync_entry_to_device(state, pasteboard_id, &d.instance_id, false).await {
                if e == "offline" {
                    let _ = state.with_store(|s| s.enqueue_sync(pasteboard_id, &d.instance_id));
                }
            }
        } else {
            let _ = state.with_store(|s| s.enqueue_sync(pasteboard_id, &d.instance_id));
        }
    }
    Ok(())
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
    for (pb, device_id) in pending {
        if nearby.contains_key(&device_id) {
            let _ = sync_entry_to_device(state, &pb, &device_id, false).await;
        }
    }
}

pub async fn download_file(state: &AppState, pasteboard_id: &str) -> Result<(), String> {
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

    let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "downloading", true));
    state.emit_history();

    let resp = req.send().await.map_err(|e| {
        let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "failed", true));
        map_net_err(e)
    })?;
    let status = resp.status();
    if !status.is_success() && status.as_u16() != 206 {
        let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "failed", true));
        state.emit_history();
        let t = resp.text().await.unwrap_or_default();
        if status.as_u16() == 401 || t.contains("device_removed") {
            return Err("device_removed".into());
        }
        if status.as_u16() == 403 || t.contains("rejected") {
            return Err("rejected".into());
        }
        return Err(if t.is_empty() {
            "source_file_gone".into()
        } else {
            t
        });
    }
    let total = resp.content_length().or(file.file_size).unwrap_or(0);
    let mut received = 0u64;
    let mut data = Vec::new();
    let mut stream = resp.bytes_stream();
    let handle = state.handle().ok();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            let _ = state.with_store(|s| s.update_download_state(pasteboard_id, "failed", true));
            e.to_string()
        })?;
        received += chunk.len() as u64;
        data.extend_from_slice(&chunk);
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

    state.with_store(|s| {
        let (hash, size) = s.put_blob(&data)?;
        let mut preview: crate::types::Preview =
            serde_json::from_str(&pb.preview_json).unwrap_or_default();
        preview.path = Some(
            s.named_blob_path(&hash, file.file_name.as_deref())
                .unwrap_or_else(|_| s.blob_path(&hash))
                .to_string_lossy()
                .into_owned(),
        );
        preview.file_size = Some(size);
        if crate::clipboard::looks_like_image_name(file.file_name.as_deref().unwrap_or(""))
            || crate::clipboard::looks_like_image_magic(&data)
        {
            crate::clipboard::apply_image_thumb(&mut preview, &data);
        }
        let preview_json = serde_json::to_string(&preview).unwrap_or_else(|_| "{}".into());
        s.attach_blob_to_file_item(pasteboard_id, &hash, size, &preview_json)?;
        Ok(())
    })?;
    state.emit_history();
    Ok(())
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
}
