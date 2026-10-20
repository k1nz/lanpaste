use std::collections::{HashMap, HashSet};

use rand::RngCore;
use subtle::ConstantTimeEq;

/// Eight decimal digits. Combined with the attempt cap below, an online
/// guesser who already holds a paired device key still cannot walk the space.
pub const CODE_LEN: usize = 8;
pub const CODE_TTL_MS: i64 = 10 * 60 * 1000;
pub const LOCKOUT_MS: i64 = 15 * 60 * 1000;
pub const MAX_FAILURES: u32 = 5;
pub const OFFLINE_GRACE_MS: i64 = 45_000;

const CODE_SPACE: u32 = 100_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCode {
    pub instance_id: String,
    pub device_name: String,
    pub code: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    pub open: bool,
    /// Set only when this call minted a code. Repeat requests must not pop the
    /// window again or rotate the code.
    pub minted: Option<PendingCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockFail {
    Invalid,
    Expired,
    LockedOut,
}

impl UnlockFail {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "session_code_invalid",
            Self::Expired => "session_code_expired",
            Self::LockedOut => "session_locked_out",
        }
    }
}

#[derive(Debug, Clone)]
struct Peer {
    device_name: String,
    code: Option<String>,
    expires_at: i64,
    failures: u32,
    locked_until: i64,
    granted: bool,
    last_authed_at: Option<i64>,
    offline_since: Option<i64>,
}

impl Peer {
    fn new(name: &str) -> Self {
        Self {
            device_name: name.to_string(),
            code: None,
            expires_at: 0,
            failures: 0,
            locked_until: 0,
            granted: false,
            last_authed_at: None,
            offline_since: None,
        }
    }
}

/// In-memory per-peer session challenges and grants. Nothing here is written
/// to disk: a restart, or the peer leaving the LAN, requires a new code.
#[derive(Debug, Default)]
pub struct SessionBook {
    peers: HashMap<String, Peer>,
}

impl SessionBook {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// Decide whether `instance_id` may use the data plane right now.
    /// A signed request also proves the peer is still reachable, so the offline
    /// countdown is cancelled even when the grant is still closed.
    pub fn authorize(&mut self, instance_id: &str, device_name: &str, now: i64) -> Gate {
        let peer = self
            .peers
            .entry(instance_id.to_string())
            .or_insert_with(|| Peer::new(device_name));
        peer.device_name = device_name.to_string();
        peer.last_authed_at = Some(now);
        peer.offline_since = None;

        if peer.granted {
            return Gate {
                open: true,
                minted: None,
            };
        }
        if peer.locked_until > now {
            return Gate {
                open: false,
                minted: None,
            };
        }
        if peer.locked_until > 0 {
            peer.locked_until = 0;
            peer.failures = 0;
        }
        if peer.code.is_some() && now <= peer.expires_at {
            return Gate {
                open: false,
                minted: None,
            };
        }

        let code = generate_session_code();
        let expires_at = now.saturating_add(CODE_TTL_MS);
        peer.code = Some(code.clone());
        peer.expires_at = expires_at;
        Gate {
            open: false,
            minted: Some(PendingCode {
                instance_id: instance_id.to_string(),
                device_name: device_name.to_string(),
                code,
                expires_at,
            }),
        }
    }

    pub fn unlock(&mut self, instance_id: &str, code: &str, now: i64) -> Result<(), UnlockFail> {
        let Some(peer) = self.peers.get_mut(instance_id) else {
            return Err(UnlockFail::Expired);
        };
        peer.last_authed_at = Some(now);
        peer.offline_since = None;
        if peer.granted {
            return Ok(());
        }
        if peer.locked_until > now {
            return Err(UnlockFail::LockedOut);
        }
        if peer.locked_until > 0 {
            peer.locked_until = 0;
            peer.failures = 0;
        }
        let Some(expected) = peer.code.clone() else {
            return Err(UnlockFail::Expired);
        };
        if now > peer.expires_at {
            return Err(UnlockFail::Expired);
        }
        if codes_match(&expected, code) {
            peer.code = None;
            peer.expires_at = 0;
            peer.failures = 0;
            peer.locked_until = 0;
            peer.granted = true;
            return Ok(());
        }
        peer.failures = peer.failures.saturating_add(1);
        if peer.failures >= MAX_FAILURES {
            peer.locked_until = now.saturating_add(LOCKOUT_MS);
            peer.code = None;
            peer.expires_at = 0;
            return Err(UnlockFail::LockedOut);
        }
        Err(UnlockFail::Invalid)
    }

    pub fn is_granted(&self, instance_id: &str) -> bool {
        self.peers.get(instance_id).is_some_and(|p| p.granted)
    }

    pub fn pending(&self, now: i64) -> Vec<PendingCode> {
        let mut out = Vec::new();
        for (id, peer) in &self.peers {
            let Some(code) = peer.code.clone() else {
                continue;
            };
            if now > peer.expires_at {
                continue;
            }
            out.push(PendingCode {
                instance_id: id.clone(),
                device_name: peer.device_name.clone(),
                code,
                expires_at: peer.expires_at,
            });
        }
        out.sort_by(|a, b| {
            a.device_name
                .cmp(&b.device_name)
                .then(a.instance_id.cmp(&b.instance_id))
        });
        out
    }

    /// Drop grants and outstanding codes once a peer has been absent from mDNS
    /// and silent for [`OFFLINE_GRACE_MS`]. Failure counters and lockouts stay,
    /// so leaving the LAN is not a way to reset them.
    pub fn expire_offline(&mut self, now: i64, online: &HashSet<String>) -> bool {
        let mut changed = false;
        for (id, peer) in &mut self.peers {
            if online.contains(id) {
                peer.offline_since = None;
                continue;
            }
            if peer.offline_since.is_none() {
                peer.offline_since = Some(now);
            }
            let since = peer.offline_since.unwrap_or(now);
            let recent = peer
                .last_authed_at
                .is_some_and(|t| now.saturating_sub(t) < OFFLINE_GRACE_MS);
            if recent || now.saturating_sub(since) < OFFLINE_GRACE_MS {
                continue;
            }
            if peer.granted || peer.code.is_some() {
                peer.granted = false;
                peer.code = None;
                peer.expires_at = 0;
                changed = true;
            }
        }
        changed
    }
}

pub fn generate_session_code() -> String {
    // Rejection sampling keeps the 8-digit space uniform. `u32 % 1e8` alone
    // would slightly prefer the low residues.
    let limit = u32::MAX - (u32::MAX % CODE_SPACE);
    loop {
        let mut buf = [0u8; 4];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let n = u32::from_le_bytes(buf);
        if n < limit {
            return format!("{:08}", n % CODE_SPACE);
        }
    }
}

pub fn normalize_code(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn codes_match(expected: &str, input: &str) -> bool {
    let input = normalize_code(input);
    let mut exp = [0u8; CODE_LEN];
    let mut got = [0u8; CODE_LEN];
    let exp_ok = copy_exact(expected.as_bytes(), &mut exp);
    let got_ok = copy_exact(input.as_bytes(), &mut got);
    let bytes_eq = exp.ct_eq(&got);
    let lens_ok = subtle::Choice::from((exp_ok & got_ok) as u8);
    bool::from(bytes_eq & lens_ok)
}

fn copy_exact(src: &[u8], dst: &mut [u8; CODE_LEN]) -> bool {
    if src.len() != CODE_LEN {
        return false;
    }
    dst.copy_from_slice(src);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn online(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn generated_code_is_eight_digits() {
        for _ in 0..32 {
            let code = generate_session_code();
            assert_eq!(code.len(), CODE_LEN);
            assert!(code.bytes().all(|b| b.is_ascii_digit()));
        }
    }

    #[test]
    fn repeat_authorize_does_not_rotate_or_remint() {
        let mut book = SessionBook::new();
        let first = book.authorize("a", "Alpha", 1_000);
        assert!(!first.open);
        let code = first.minted.unwrap().code;
        let second = book.authorize("a", "Alpha", 1_000 + 5_000);
        assert!(!second.open);
        assert!(second.minted.is_none());
        assert_eq!(book.pending(2_000)[0].code, code);
    }

    #[test]
    fn wrong_code_then_right_code_binds_the_peer() {
        let mut book = SessionBook::new();
        let minted = book.authorize("a", "Alpha", 0).minted.unwrap();
        assert_eq!(
            book.unlock("b", &minted.code, 10).unwrap_err(),
            UnlockFail::Expired
        );
        assert!(!book.is_granted("b"));
        assert_eq!(
            book.unlock("a", "00000000", 10).unwrap_err(),
            UnlockFail::Invalid
        );
        book.unlock(
            "a",
            &format!("{} {}", &minted.code[..4], &minted.code[4..]),
            20,
        )
        .unwrap();
        assert!(book.is_granted("a"));
        assert!(!book.is_granted("b"));
        assert!(book.pending(30).is_empty());
        // A repeat from the same device is a no-op. The code itself is gone,
        // so it cannot mint a grant for anyone else.
        book.unlock("a", &minted.code, 40).unwrap();
        assert!(book.authorize("a", "Alpha", 50).open);
        assert!(book.authorize("a", "Alpha", 60).minted.is_none());
    }

    #[test]
    fn fifth_failure_locks_without_minting_a_replacement() {
        let mut book = SessionBook::new();
        let code = book.authorize("a", "Alpha", 0).minted.unwrap().code;
        for _ in 0..4 {
            assert_eq!(
                book.unlock("a", "11111111", 10).unwrap_err(),
                UnlockFail::Invalid
            );
        }
        assert_eq!(book.pending(10)[0].code, code);
        assert_eq!(
            book.unlock("a", "11111111", 20).unwrap_err(),
            UnlockFail::LockedOut
        );
        assert!(book.pending(30).is_empty());
        let during = book.authorize("a", "Alpha", 30);
        assert!(!during.open);
        assert!(during.minted.is_none());
        assert_eq!(
            book.unlock("a", &code, 40).unwrap_err(),
            UnlockFail::LockedOut
        );
        let after = book.authorize("a", "Alpha", LOCKOUT_MS + 20);
        assert!(after.minted.is_some());
        assert_ne!(after.minted.unwrap().code, code);
    }

    #[test]
    fn failures_survive_code_rotation() {
        let mut book = SessionBook::new();
        let first = book.authorize("a", "Alpha", 0).minted.unwrap().code;
        for _ in 0..4 {
            let _ = book.unlock("a", "11111111", 10);
        }
        let second = book
            .authorize("a", "Alpha", CODE_TTL_MS + 1)
            .minted
            .unwrap()
            .code;
        assert_ne!(first, second);
        assert_eq!(
            book.unlock("a", "11111111", CODE_TTL_MS + 2).unwrap_err(),
            UnlockFail::LockedOut
        );
    }

    #[test]
    fn expired_code_does_not_count_as_a_guess() {
        let mut book = SessionBook::new();
        let code = book.authorize("a", "Alpha", 0).minted.unwrap().code;
        assert_eq!(
            book.unlock("a", &code, CODE_TTL_MS + 1).unwrap_err(),
            UnlockFail::Expired
        );
        let minted = book.authorize("a", "Alpha", CODE_TTL_MS + 2).minted;
        assert!(minted.is_some());
        book.unlock("a", &minted.unwrap().code, CODE_TTL_MS + 3)
            .unwrap();
    }

    #[test]
    fn offline_grace_revokes_grant() {
        let mut book = SessionBook::new();
        let code = book.authorize("a", "Alpha", 0).minted.unwrap().code;
        book.unlock("a", &code, 1_000).unwrap();
        assert!(!book.expire_offline(1_000, &online(&[])));
        assert!(book.is_granted("a"));
        assert!(!book.expire_offline(1_000 + OFFLINE_GRACE_MS - 1, &online(&[])));
        assert!(book.is_granted("a"));
        assert!(book.expire_offline(1_000 + OFFLINE_GRACE_MS, &online(&[])));
        assert!(!book.is_granted("a"));

        let again = book.authorize("a", "Alpha", 5_000).minted.unwrap().code;
        book.unlock("a", &again, 5_100).unwrap();
        assert!(!book.expire_offline(5_100 + OFFLINE_GRACE_MS, &online(&["a"])));
        assert!(book.is_granted("a"));
    }

    #[test]
    fn lockout_survives_going_offline() {
        let mut book = SessionBook::new();
        let _ = book.authorize("a", "Alpha", 0).minted.unwrap();
        for _ in 0..4 {
            assert_eq!(
                book.unlock("a", "00000000", 10).unwrap_err(),
                UnlockFail::Invalid
            );
        }
        assert_eq!(
            book.unlock("a", "00000000", 20).unwrap_err(),
            UnlockFail::LockedOut
        );
        assert!(!book.expire_offline(20 + OFFLINE_GRACE_MS, &online(&[])));
        let blocked = book.authorize("a", "Alpha", 30 + OFFLINE_GRACE_MS);
        assert!(blocked.minted.is_none());
        assert!(!blocked.open);
    }

    #[test]
    fn recent_signed_request_cancels_the_offline_countdown() {
        let mut book = SessionBook::new();
        let code = book.authorize("a", "Alpha", 0).minted.unwrap().code;
        book.unlock("a", &code, 0).unwrap();
        assert!(!book.expire_offline(0, &online(&[])));
        let almost = OFFLINE_GRACE_MS - 1;
        assert!(!book.expire_offline(almost, &online(&[])));
        assert!(book.is_granted("a"));
        assert!(book.authorize("a", "Alpha", almost).open);
        assert!(!book.expire_offline(almost, &online(&[])));
        assert!(!book.expire_offline(almost + OFFLINE_GRACE_MS - 1, &online(&[])));
        assert!(book.is_granted("a"));
        assert!(book.expire_offline(almost + OFFLINE_GRACE_MS, &online(&[])));
        assert!(!book.is_granted("a"));
    }
}
