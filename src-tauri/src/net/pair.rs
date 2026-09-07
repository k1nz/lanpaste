use crate::state::IncomingPair;
use crate::types::PAIR_TOKEN_TTL_MS;

pub const TOKEN_WRONG: &str = "token_invalid";
pub const TOKEN_EXPIRED: &str = "token_expired";
pub const TOKEN_USED: &str = "token_invalid";

pub fn validate_token(pending: &IncomingPair, now_ms: i64, input: &str) -> Result<(), String> {
    if pending.used {
        return Err(TOKEN_USED.into());
    }
    if now_ms > pending.expires_at {
        return Err(TOKEN_EXPIRED.into());
    }
    if pending.token != input.trim() {
        return Err(TOKEN_WRONG.into());
    }
    Ok(())
}

pub fn token_expires_at(now_ms: i64) -> i64 {
    now_ms + PAIR_TOKEN_TTL_MS as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(token: &str, expires_at: i64, used: bool) -> IncomingPair {
        IncomingPair {
            peer_instance_id: "peer".into(),
            peer_name: "B".into(),
            peer_fingerprint: "fp".into(),
            peer_public_key: "pk".into(),
            peer_cert_der: "cert".into(),
            token: token.into(),
            expires_at,
            used,
        }
    }

    #[test]
    fn token_wrong() {
        let p = pending("123456", 10_000, false);
        assert_eq!(validate_token(&p, 1_000, "000000").unwrap_err(), TOKEN_WRONG);
    }

    #[test]
    fn token_expired() {
        let p = pending("123456", 1_000, false);
        assert_eq!(validate_token(&p, 2_000, "123456").unwrap_err(), TOKEN_EXPIRED);
    }

    #[test]
    fn token_one_time() {
        let p = pending("123456", 10_000, true);
        assert_eq!(validate_token(&p, 1_000, "123456").unwrap_err(), TOKEN_USED);
        let p = pending("123456", 10_000, false);
        assert!(validate_token(&p, 1_000, "123456").is_ok());
    }
}
