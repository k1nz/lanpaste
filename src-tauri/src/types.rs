use serde::{Deserialize, Serialize};

pub const DEFAULT_AUTO_SYNC_MAX_BYTES: u64 = 20 * 1024 * 1024;
pub const DEFAULT_CLEANUP_MAX_ITEMS: u64 = 500;
pub const DEFAULT_CLEANUP_MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_SHORTCUT: &str = "Option+Shift+V";
pub const PAIR_TOKEN_TTL_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteType {
    Text,
    Url,
    Color,
    Html,
    Rtf,
    Image,
    File,
}

impl PasteType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Url => "url",
            Self::Color => "color",
            Self::Html => "html",
            Self::Rtf => "rtf",
            Self::Image => "image",
            Self::File => "file",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "url" => Some(Self::Url),
            "color" => Some(Self::Color),
            "html" => Some(Self::Html),
            "rtf" => Some(Self::Rtf),
            "image" => Some(Self::Image),
            "file" => Some(Self::File),
            _ => None,
        }
    }
}

/// Primary appearance type: file > color > html > rtf > image > url > text.
pub fn primary_type(types: &[PasteType]) -> PasteType {
    const ORDER: &[PasteType] = &[
        PasteType::File,
        PasteType::Color,
        PasteType::Html,
        PasteType::Rtf,
        PasteType::Image,
        PasteType::Url,
        PasteType::Text,
    ];
    for candidate in ORDER {
        if types.contains(candidate) {
            return *candidate;
        }
    }
    PasteType::Text
}

pub fn should_auto_sync(total_bytes: u64, max_bytes: u64) -> bool {
    total_bytes <= max_bytes
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_thumb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub copied_at: i64,
    pub source_device_id: Option<String>,
    pub source_device_name: String,
    pub primary_type: PasteType,
    pub title: String,
    pub preview: Preview,
    pub needs_file_download: bool,
    pub file_download_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearbyDevice {
    pub instance_id: String,
    pub name: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDevice {
    pub instance_id: String,
    pub name: String,
    pub note: String,
    pub online: bool,
    pub allow_send: bool,
    pub allow_receive: bool,
    pub auto_write_clipboard: bool,
    pub trust_broken: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub auto_sync_max_bytes: u64,
    pub cleanup_max_items: u64,
    pub cleanup_max_bytes: u64,
    pub cleanup_max_age_days: Option<u32>,
    pub overlay_shortcut: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_sync_max_bytes: DEFAULT_AUTO_SYNC_MAX_BYTES,
            cleanup_max_items: DEFAULT_CLEANUP_MAX_ITEMS,
            cleanup_max_bytes: DEFAULT_CLEANUP_MAX_BYTES,
            cleanup_max_age_days: None,
            overlay_shortcut: DEFAULT_SHORTCUT.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingShowPayload {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInputPayload {
    pub instance_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgressPayload {
    pub id: String,
    pub received: u64,
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_type_order() {
        assert_eq!(
            primary_type(&[PasteType::Text, PasteType::File]),
            PasteType::File
        );
        assert_eq!(
            primary_type(&[PasteType::Text, PasteType::Color]),
            PasteType::Color
        );
        assert_eq!(
            primary_type(&[PasteType::Text, PasteType::Html, PasteType::Rtf]),
            PasteType::Html
        );
        assert_eq!(
            primary_type(&[PasteType::Image, PasteType::Url, PasteType::Text]),
            PasteType::Image
        );
        assert_eq!(primary_type(&[PasteType::Text]), PasteType::Text);
        assert_eq!(primary_type(&[]), PasteType::Text);
    }

    #[test]
    fn auto_sync_20mb_threshold() {
        let cap = DEFAULT_AUTO_SYNC_MAX_BYTES;
        assert!(should_auto_sync(0, cap));
        assert!(should_auto_sync(cap, cap));
        assert!(!should_auto_sync(cap + 1, cap));
        assert!(!should_auto_sync(50 * 1024 * 1024, cap));
    }
}
