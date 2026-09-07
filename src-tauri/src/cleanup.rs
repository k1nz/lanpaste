use std::collections::HashSet;

use crate::store::{now_ms, Store};
use crate::types::AppSettings;

/// Delete oldest pasteboards until under item/byte/age limits.
/// Skip any pasteboard whose blobs are currently being served via GET.
pub fn apply_cleanup(
    store: &mut Store,
    settings: &AppSettings,
    inflight: &HashSet<String>,
) -> Result<usize, String> {
    let mut deleted = 0usize;
    let max_age_ms = settings.cleanup_max_age_days.map(|d| d as i64 * 86_400_000);
    let now = now_ms();

    loop {
        let (count, bytes) = store.stats()?;
        let oldest = store.list_oldest()?;
        let mut victim = None;
        for pb in &oldest {
            let hashes = store.pasteboard_blob_hashes(&pb.id)?;
            if hashes.iter().any(|h| inflight.contains(h)) {
                continue;
            }
            let too_old = max_age_ms
                .map(|max| now.saturating_sub(pb.copied_at) > max)
                .unwrap_or(false);
            let over_count = count > settings.cleanup_max_items;
            let over_bytes = bytes > settings.cleanup_max_bytes;
            if too_old || over_count || over_bytes {
                victim = Some(pb.id.clone());
                break;
            }
        }
        match victim {
            Some(id) => {
                store.delete_pasteboard(&id)?;
                deleted += 1;
            }
            None => break,
        }
        if deleted > 10_000 {
            break;
        }
    }
    Ok(deleted)
}

/// Pure helper used by tests: given oldest-first ids + sizes, return deletion order.
pub fn cleanup_order(
    oldest_first: &[(String, u64, i64)],
    max_items: u64,
    max_bytes: u64,
    max_age_days: Option<u32>,
    now_ms: i64,
    inflight_ids: &HashSet<String>,
) -> Vec<String> {
    let mut remaining: Vec<(String, u64, i64)> = oldest_first.to_vec();
    let mut deleted = Vec::new();
    loop {
        let count = remaining.len() as u64;
        let bytes: u64 = remaining.iter().map(|(_, s, _)| *s).sum();
        let mut victim = None;
        for (id, _, copied_at) in &remaining {
            if inflight_ids.contains(id) {
                continue;
            }
            let too_old = max_age_days
                .map(|d| now_ms.saturating_sub(*copied_at) > d as i64 * 86_400_000)
                .unwrap_or(false);
            if too_old || count > max_items || bytes > max_bytes {
                victim = Some(id.clone());
                break;
            }
        }
        match victim {
            Some(id) => {
                remaining.retain(|(x, _, _)| x != &id);
                deleted.push(id);
            }
            None => break,
        }
    }
    deleted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_oldest_first_and_skip_inflight() {
        let items = vec![
            ("a".into(), 100u64, 1_000i64),
            ("b".into(), 100, 2_000),
            ("c".into(), 100, 3_000),
        ];
        let mut inflight = HashSet::new();
        inflight.insert("a".into());
        // Keep 2: skip inflight "a", drop next-oldest "b", leave "c".
        let deleted = cleanup_order(&items, 2, 10_000, None, 10_000, &inflight);
        assert_eq!(deleted, vec!["b".to_string()]);
    }

    #[test]
    fn cleanup_whichever_limit_first() {
        let items: Vec<(String, u64, i64)> = (0..10)
            .map(|i| (format!("n{i}"), 100u64, i as i64))
            .collect();
        let deleted = cleanup_order(&items, 3, 10_000, None, 100, &HashSet::new());
        assert_eq!(deleted.len(), 7);
        assert_eq!(deleted[0], "n0");
        assert_eq!(deleted[6], "n6");

        let big = vec![
            ("x".into(), 900u64, 1),
            ("y".into(), 200, 2),
            ("z".into(), 50, 3),
        ];
        let deleted = cleanup_order(&big, 50, 200, None, 100, &HashSet::new());
        assert_eq!(deleted, vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn cleanup_max_age() {
        let items = vec![
            ("old".into(), 10u64, 0i64),
            ("new".into(), 10, 90_000_000),
        ];
        let deleted = cleanup_order(&items, 500, 1_000_000, Some(1), 90_000_000, &HashSet::new());
        assert_eq!(deleted, vec!["old".to_string()]);
    }
}
