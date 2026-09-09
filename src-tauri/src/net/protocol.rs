use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::store::content_hash_for;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    pub instance_id: String,
    pub name: String,
    pub fingerprint: String,
    pub public_key: String,
    pub cert_der: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairConfirm {
    pub instance_id: String,
    pub name: String,
    pub token: String,
    pub fingerprint: String,
    pub public_key: String,
    pub cert_der: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairConfirmResponse {
    pub instance_id: String,
    pub name: String,
    pub fingerprint: String,
    pub public_key: String,
    pub cert_der: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRevoke {
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEntryBody {
    pub id: String,
    pub copied_at: i64,
    pub source_device_id: String,
    pub source_device_name: String,
    pub primary_type: String,
    pub title: String,
    pub total_bytes: u64,
    #[serde(default)]
    pub content_hash: String,
    pub items: Vec<SyncItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtf_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Image bytes (base64). Never used for files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

pub fn file_item_has_no_bytes(item: &SyncItem) -> bool {
    item.item_type != "file" || (item.image_b64.is_none())
}

pub fn resolved_content_hash(body: &SyncEntryBody) -> String {
    if !body.content_hash.is_empty() && !body.content_hash.starts_with("sync:") {
        return body.content_hash.clone();
    }
    content_hash_of_sync_items(&body.items)
}

pub fn content_hash_of_sync_items(items: &[SyncItem]) -> String {
    let mut parts = Vec::with_capacity(items.len());
    for it in items {
        let bytes = match it.item_type.as_str() {
            "file" => it
                .file_hash
                .as_deref()
                .and_then(|h| hex::decode(h).ok())
                .unwrap_or_else(|| {
                    it.file_name
                        .as_deref()
                        .unwrap_or_default()
                        .as_bytes()
                        .to_vec()
                }),
            "image" => it
                .image_b64
                .as_deref()
                .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
                .map(|raw| Sha256::digest(&raw).to_vec())
                .unwrap_or_default(),
            "rtf" => it
                .rtf_b64
                .as_deref()
                .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
                .unwrap_or_default(),
            "html" => it.html.as_deref().unwrap_or_default().as_bytes().to_vec(),
            "url" => it.url.as_deref().unwrap_or_default().as_bytes().to_vec(),
            "color" => it.color.as_deref().unwrap_or_default().as_bytes().to_vec(),
            _ => it.text.as_deref().unwrap_or_default().as_bytes().to_vec(),
        };
        parts.push((it.item_type.clone(), bytes));
    }
    content_hash_for(&parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_hash_prefers_body_field() {
        let body = SyncEntryBody {
            id: "p1".into(),
            copied_at: 1,
            source_device_id: "a".into(),
            source_device_name: "A".into(),
            primary_type: "text".into(),
            title: "hi".into(),
            total_bytes: 2,
            content_hash: "abc123".into(),
            items: vec![SyncItem {
                id: "i1".into(),
                item_type: "text".into(),
                text: Some("hi".into()),
                ..Default::default()
            }],
        };
        assert_eq!(resolved_content_hash(&body), "abc123");
    }

    #[test]
    fn text_sync_hash_matches_store_hasher() {
        let items = vec![SyncItem {
            id: "i1".into(),
            item_type: "text".into(),
            text: Some("hello".into()),
            ..Default::default()
        }];
        assert_eq!(
            content_hash_of_sync_items(&items),
            content_hash_for(&[("text".into(), b"hello".to_vec())])
        );
    }

    #[test]
    fn empty_body_hash_falls_back_to_items() {
        let body = SyncEntryBody {
            id: "p1".into(),
            copied_at: 1,
            source_device_id: "a".into(),
            source_device_name: "A".into(),
            primary_type: "text".into(),
            title: "hello".into(),
            total_bytes: 5,
            content_hash: String::new(),
            items: vec![SyncItem {
                id: "i1".into(),
                item_type: "text".into(),
                text: Some("hello".into()),
                ..Default::default()
            }],
        };
        assert_eq!(
            resolved_content_hash(&body),
            content_hash_for(&[("text".into(), b"hello".to_vec())])
        );
        let mut legacy = body.clone();
        legacy.content_hash = "sync:p1".into();
        assert_eq!(
            resolved_content_hash(&legacy),
            content_hash_for(&[("text".into(), b"hello".to_vec())])
        );
    }
}
