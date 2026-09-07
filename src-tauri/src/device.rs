use std::collections::HashMap;

use crate::store::StoredDevice;
use crate::types::{NearbyDevice, PairedDevice};

#[derive(Debug, Clone)]
pub struct NearbyInfo {
    pub instance_id: String,
    pub name: String,
    pub fingerprint: String,
    pub host: String,
    pub port: u16,
}

pub fn nearby_list(
    nearby: &HashMap<String, NearbyInfo>,
    paired_ids: &[String],
    self_id: &str,
) -> Vec<NearbyDevice> {
    nearby
        .values()
        .filter(|n| n.instance_id != self_id && !paired_ids.iter().any(|p| p == &n.instance_id))
        .map(|n| NearbyDevice {
            instance_id: n.instance_id.clone(),
            name: n.name.clone(),
            fingerprint: n.fingerprint.clone(),
        })
        .collect()
}

pub fn paired_list(devices: &[StoredDevice], nearby: &HashMap<String, NearbyInfo>) -> Vec<PairedDevice> {
    devices
        .iter()
        .map(|d| {
            let online = nearby.contains_key(&d.instance_id)
                || d.host.as_ref().zip(d.port).is_some_and(|_| {
                    nearby
                        .get(&d.instance_id)
                        .map(|n| n.port == d.port.unwrap_or(n.port))
                        .unwrap_or(false)
                });
            let online = online || nearby.contains_key(&d.instance_id);
            PairedDevice {
                instance_id: d.instance_id.clone(),
                name: d.name.clone(),
                note: d.note.clone(),
                online,
                allow_send: d.allow_send,
                allow_receive: d.allow_receive,
                auto_write_clipboard: d.auto_write_clipboard,
                trust_broken: d.trust_broken,
            }
        })
        .collect()
}

pub fn local_device_name() -> String {
    if let Ok(out) = std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
    {
        if out.status.success() {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Mac".into())
}
