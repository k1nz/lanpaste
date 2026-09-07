use std::collections::HashMap;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::device::NearbyInfo;
use crate::state::AppState;
use crate::store::now_ms;

const SERVICE_TYPE: &str = "_lanpaste._tcp.local.";

pub fn start(state: AppState, port: u16) -> Result<ServiceDaemon, String> {
    let mdns = ServiceDaemon::new().map_err(|e| format!("mdns: {e}"))?;
    let instance = &state.inner.identity.instance_id;
    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "lanpaste".into());
    let host_name = if host.ends_with(".local.") {
        host
    } else {
        format!("{host}.local.")
    };
    let inst_name = instance.chars().take(15).collect::<String>();
    let mut props = HashMap::new();
    props.insert("id".to_string(), instance.clone());
    props.insert("name".to_string(), state.inner.device_name.clone());
    props.insert("fp".to_string(), state.inner.identity.fingerprint.clone());

    let mut info = ServiceInfo::new(
        SERVICE_TYPE,
        &inst_name,
        &host_name,
        "",
        port,
        Some(props),
    )
    .map_err(|e| format!("service info: {e}"))?;
    info = info.enable_addr_auto();
    mdns.register(info).map_err(|e| format!("mdns register: {e}"))?;

    let receiver = mdns
        .browse(SERVICE_TYPE)
        .map_err(|e| format!("mdns browse: {e}"))?;
    let state_browse = state.clone();
    let self_id = instance.clone();
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let props = info.get_properties();
                    let id = props
                        .get_property_val_str("id")
                        .unwrap_or("")
                        .to_string();
                    if id.is_empty() || id == self_id {
                        continue;
                    }
                    let name = props
                        .get_property_val_str("name")
                        .unwrap_or("LanPaste")
                        .to_string();
                    let fp = props.get_property_val_str("fp").unwrap_or("").to_string();
                    let host = info
                        .get_addresses_v4()
                        .iter()
                        .next()
                        .map(|ip| ip.to_string())
                        .or_else(|| {
                            info.get_hostname()
                                .trim_end_matches('.')
                                .strip_suffix(".local")
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_default();
                    let port = info.get_port();
                    if let Ok(mut g) = state_browse.inner.nearby.lock() {
                        g.insert(
                            id.clone(),
                            NearbyInfo {
                                instance_id: id.clone(),
                                name,
                                fingerprint: fp,
                                host: host.clone(),
                                port,
                            },
                        );
                    }
                    let _ = state_browse.with_store(|s| s.update_device_addr(&id, &host, port));
                    state_browse.emit_devices();
                    let _ = now_ms();
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    if let Ok(mut g) = state_browse.inner.nearby.lock() {
                        g.retain(|_, n| !fullname.contains(&n.instance_id));
                    }
                    state_browse.emit_devices();
                }
                _ => {}
            }
        }
    });
    Ok(mdns)
}
