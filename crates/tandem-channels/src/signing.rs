//! Webhook / interaction request signing verification for channel adapters.
//!
//! Each external platform uses a different signing scheme for inbound
//! interactions (button clicks, modal submits, slash commands). All of them
//! share the same operational requirements: constant-time comparison, timely
//! replay protection, and a single error type so the HTTP layer can reject
//! forged requests with a uniform 401.
//!
//! - **Slack**: HMAC-SHA256 over `v0:{timestamp}:{body}`, signed with the
//!   app's signing secret. Header `x-slack-signature` is `v0={hex}`. Reject
//!   timestamps older than 5 minutes (replay protection).
//! - **Discord**: Ed25519 signature in `x-signature-ed25519` over
//!   `{timestamp}{body}`, verified with the application's public key.
//!   (Implementation lands in W4 once `ed25519-dalek` is added; the trait
//!   shape is fixed here.)
//! - **Telegram**: per-webhook `secret_token` configured at `setWebhook` time
//!   and echoed by Telegram in the `x-telegram-bot-api-secret-token` header.
//!   Constant-time string comparison. (Wired in W4.)
//!
//! All verification functions accept the request body as `&[u8]` and return
//! `Result<(), SigningError>`. Callers should reject with HTTP 401 on error
//! and log the error kind without leaking the offending signature/payload.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Default replay-protection window for Slack-style timestamp signing.
/// Slack's published guidance is 5 minutes.
pub const SLACK_REPLAY_WINDOW_SECONDS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningError {
    /// The required signature header was absent or empty.
    MissingHeader(&'static str),
    /// The signature header was present but malformed (wrong prefix, bad hex, etc.).
    MalformedHeader(&'static str),
    /// Signature did not match the expected value computed from the body + secret.
    BadSignature,
    /// Request timestamp is outside the accepted replay window.
    StaleTimestamp { age_seconds: i64 },
    /// Timestamp header was missing or could not be parsed as i64 seconds.
    MalformedTimestamp,
    /// Configured secret was empty / not set; rejecting closed.
    SecretNotConfigured(&'static str),
}

impl core::fmt::Display for SigningError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingHeader(name) => write!(f, "missing required header: {name}"),
            Self::MalformedHeader(name) => write!(f, "malformed signature header: {name}"),
            Self::BadSignature => write!(f, "signature verification failed"),
            Self::StaleTimestamp { age_seconds } => {
                write!(f, "request timestamp is stale ({age_seconds}s old)")
            }
            Self::MalformedTimestamp => write!(f, "malformed or missing request timestamp"),
            Self::SecretNotConfigured(scheme) => {
                write!(f, "{scheme} signing secret is not configured")
            }
        }
    }
}

impl std::error::Error for SigningError {}

/// Verify a Slack signed-request signature.
///
/// Per [Slack's docs](https://api.slack.com/authentication/verifying-requests-from-slack):
///
/// 1. Concatenate `v0:` + the value of `x-slack-request-timestamp` + `:` + the raw request body.
/// 2. HMAC-SHA256 that string using the app's signing secret.
/// 3. Hex-encode the digest, prefix with `v0=`, and compare to `x-slack-signature` in
///    constant time.
///
/// Reject any request whose timestamp is more than `SLACK_REPLAY_WINDOW_SECONDS`
/// seconds away from `now_unix_seconds`. Callers in tests inject `now`; in
/// production, pass `chrono::Utc::now().timestamp()`.
pub fn verify_slack_signature(
    body: &[u8],
    signature_header: Option<&str>,
    timestamp_header: Option<&str>,
    signing_secret: &str,
    now_unix_seconds: i64,
) -> Result<(), SigningError> {
    if signing_secret.is_empty() {
        return Err(SigningError::SecretNotConfigured("slack"));
    }
    let signature = signature_header
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(SigningError::MissingHeader("x-slack-signature"))?;
    let timestamp_str = timestamp_header
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(SigningError::MissingHeader("x-slack-request-timestamp"))?;

    let timestamp: i64 = timestamp_str
        .parse()
        .map_err(|_| SigningError::MalformedTimestamp)?;
    let age = (now_unix_seconds - timestamp).abs();
    if age > SLACK_REPLAY_WINDOW_SECONDS {
        return Err(SigningError::StaleTimestamp { age_seconds: age });
    }

    let signature_hex = signature
        .strip_prefix("v0=")
        .ok_or(SigningError::MalformedHeader("x-slack-signature"))?;
    let provided =
        hex_decode(signature_hex).ok_or(SigningError::MalformedHeader("x-slack-signature"))?;

    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(signing_secret.as_bytes())
        .map_err(|_| SigningError::SecretNotConfigured("slack"))?;
    mac.update(b"v0:");
    mac.update(timestamp_str.as_bytes());
    mac.update(b":");
    mac.update(body);
    let expected = mac.finalize().into_bytes();

    if expected.ct_eq(&provided).into() {
        Ok(())
    } else {
        Err(SigningError::BadSignature)
    }
}

/// Verify a Telegram webhook secret-token header.
///
/// Telegram lets you set a per-webhook `secret_token` at `setWebhook` time;
/// every callback POST then includes `x-telegram-bot-api-secret-token` with
/// that exact value. Constant-time compare.
pub fn verify_telegram_secret_token(
    header_value: Option<&str>,
    expected_secret: &str,
) -> Result<(), SigningError> {
    if expected_secret.is_empty() {
        return Err(SigningError::SecretNotConfigured("telegram"));
    }
    let provided = header_value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(SigningError::MissingHeader(
            "x-telegram-bot-api-secret-token",
        ))?;
    let provided_bytes = provided.as_bytes();
    let expected_bytes = expected_secret.as_bytes();
    if provided_bytes.len() != expected_bytes.len() {
        return Err(SigningError::BadSignature);
    }
    if provided_bytes.ct_eq(expected_bytes).into() {
        Ok(())
    } else {
        Err(SigningError::BadSignature)
    }
}

/// Verify a Discord interaction signature (Ed25519 over `{timestamp}{body}`).
///
/// Stub: returns `SigningError::SecretNotConfigured("discord")` until the
/// `ed25519-dalek` dependency is wired in W4. The function signature is final
/// so callers can be written against it now and start passing real public keys
/// once the implementation lands.
pub fn verify_discord_signature(
    _body: &[u8],
    _signature_header: Option<&str>,
    _timestamp_header: Option<&str>,
    public_key_hex: &str,
) -> Result<(), SigningError> {
    if public_key_hex.is_empty() {
        return Err(SigningError::SecretNotConfigured("discord"));
    }
    // W4: implement Ed25519 verification using ed25519-dalek.
    // - decode public_key_hex (32 bytes)
    // - decode signature_header (64 bytes)
    // - verify_strict(public_key, timestamp_header || body, signature)
    Err(SigningError::SecretNotConfigured("discord"))
}

fn hex_decode(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compute a known-good Slack signature for use in tests.
    fn sign_slack(secret: &str, timestamp: i64, body: &str) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(b"v0:");
        mac.update(timestamp.to_string().as_bytes());
        mac.update(b":");
        mac.update(body.as_bytes());
        let digest = mac.finalize().into_bytes();
        format!("v0={}", hex_encode(&digest))
    }

    fn hex_encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    const TEST_SECRET: &str = "8f742231b10e8888abcd99yyyzz85a5";
    const TEST_BODY: &str =
        r#"{"type":"interactive_message","actions":[{"name":"approve","value":"yes"}]}"#;
    const TEST_TIMESTAMP: i64 = 1_700_000_000;

    #[test]
    fn slack_accepts_valid_signature() {
        let sig = sign_slack(TEST_SECRET, TEST_TIMESTAMP, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP + 30,
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn slack_rejects_missing_signature_header() {
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            None,
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(
            result,
            Err(SigningError::MissingHeader("x-slack-signature"))
        );
    }

    #[test]
    fn slack_rejects_missing_timestamp_header() {
        let sig = sign_slack(TEST_SECRET, TEST_TIMESTAMP, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            None,
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(
            result,
            Err(SigningError::MissingHeader("x-slack-request-timestamp"))
        );
    }

    #[test]
    fn slack_rejects_malformed_signature_prefix() {
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some("garbage-no-prefix"),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(
            result,
            Err(SigningError::MalformedHeader("x-slack-signature"))
        );
    }

    #[test]
    fn slack_rejects_non_hex_signature() {
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some("v0=zzzz"),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(
            result,
            Err(SigningError::MalformedHeader("x-slack-signature"))
        );
    }

    #[test]
    fn slack_rejects_forged_signature() {
        let bad_sig = format!("v0={}", "a".repeat(64));
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&bad_sig),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(result, Err(SigningError::BadSignature));
    }

    #[test]
    fn slack_rejects_signature_with_different_secret() {
        let sig = sign_slack("wrong-secret", TEST_TIMESTAMP, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(result, Err(SigningError::BadSignature));
    }

    #[test]
    fn slack_rejects_signature_with_modified_body() {
        let sig = sign_slack(TEST_SECRET, TEST_TIMESTAMP, TEST_BODY);
        let modified_body = format!("{TEST_BODY} tampered");
        let result = verify_slack_signature(
            modified_body.as_bytes(),
            Some(&sig),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(result, Err(SigningError::BadSignature));
    }

    #[test]
    fn slack_rejects_stale_timestamp_past() {
        let sig = sign_slack(TEST_SECRET, TEST_TIMESTAMP, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP + SLACK_REPLAY_WINDOW_SECONDS + 1,
        );
        assert!(matches!(result, Err(SigningError::StaleTimestamp { .. })));
    }

    #[test]
    fn slack_rejects_stale_timestamp_future() {
        let future_ts = TEST_TIMESTAMP + SLACK_REPLAY_WINDOW_SECONDS + 100;
        let sig = sign_slack(TEST_SECRET, future_ts, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            Some(&future_ts.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert!(matches!(result, Err(SigningError::StaleTimestamp { .. })));
    }

    #[test]
    fn slack_accepts_timestamp_at_replay_boundary() {
        let sig = sign_slack(TEST_SECRET, TEST_TIMESTAMP, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            Some(&TEST_TIMESTAMP.to_string()),
            TEST_SECRET,
            TEST_TIMESTAMP + SLACK_REPLAY_WINDOW_SECONDS,
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn slack_rejects_malformed_timestamp() {
        let sig = sign_slack(TEST_SECRET, TEST_TIMESTAMP, TEST_BODY);
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some(&sig),
            Some("not-a-timestamp"),
            TEST_SECRET,
            TEST_TIMESTAMP,
        );
        assert_eq!(result, Err(SigningError::MalformedTimestamp));
    }

    #[test]
    fn slack_rejects_empty_secret() {
        let result = verify_slack_signature(
            TEST_BODY.as_bytes(),
            Some("v0=00"),
            Some(&TEST_TIMESTAMP.to_string()),
            "",
            TEST_TIMESTAMP,
        );
        assert_eq!(result, Err(SigningError::SecretNotConfigured("slack")));
    }

    #[test]
    fn telegram_accepts_matching_token() {
        assert_eq!(
            verify_telegram_secret_token(Some("supersecret"), "supersecret"),
            Ok(())
        );
    }

    #[test]
    fn telegram_rejects_mismatched_token() {
        assert_eq!(
            verify_telegram_secret_token(Some("wrong"), "supersecret"),
            Err(SigningError::BadSignature)
        );
    }

    #[test]
    fn telegram_rejects_different_length_token() {
        assert_eq!(
            verify_telegram_secret_token(Some("short"), "supersecret"),
            Err(SigningError::BadSignature)
        );
    }

    #[test]
    fn telegram_rejects_missing_header() {
        assert_eq!(
            verify_telegram_secret_token(None, "supersecret"),
            Err(SigningError::MissingHeader(
                "x-telegram-bot-api-secret-token"
            ))
        );
    }

    #[test]
    fn telegram_rejects_empty_secret() {
        assert_eq!(
            verify_telegram_secret_token(Some("anything"), ""),
            Err(SigningError::SecretNotConfigured("telegram"))
        );
    }

    #[test]
    fn discord_stub_returns_secret_not_configured_when_key_missing() {
        let result = verify_discord_signature(b"body", Some("sig"), Some("ts"), "");
        assert_eq!(result, Err(SigningError::SecretNotConfigured("discord")));
    }

    #[test]
    fn discord_stub_returns_secret_not_configured_pending_w4() {
        // Until the W4 ed25519-dalek dependency lands, this verification is a stub.
        // The contract is fixed: Err(SecretNotConfigured("discord")) means callers
        // should disable Discord interactions, not silently allow them.
        let result = verify_discord_signature(b"body", Some("sig"), Some("ts"), "deadbeef");
        assert_eq!(result, Err(SigningError::SecretNotConfigured("discord")));
    }

    #[test]
    fn hex_decode_handles_uppercase_lowercase_mixed() {
        let cases = [
            ("00", vec![0]),
            ("ff", vec![255]),
            ("Ff", vec![255]),
            ("aA", vec![170]),
        ];
        for (input, expected) in cases {
            assert_eq!(hex_decode(input), Some(expected.clone()), "input {input}");
        }
    }

    #[test]
    fn hex_decode_rejects_odd_length_and_invalid_chars() {
        assert!(hex_decode("a").is_none());
        assert!(hex_decode("zz").is_none());
        assert!(hex_decode("1g").is_none());
    }
}
