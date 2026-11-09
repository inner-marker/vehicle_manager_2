//! A paired desktop's signed requests (phase-3.md 2b, decision #073; sub-plan 04 §6).
//!
//! A paired desktop holds an Ed25519 key; the server has its public half. Each request carries
//! [`HEADER`], so a copied session token is useless without the key. The format is our own,
//! shaped like `DPoP` (RFC 9449) without its JWTs, since only our own apps send it:
//!
//! ```text
//! x-device-signature: v1;scope=request;t=1762700000000;n=<32 hex>;s=<128 hex>
//! ```
//!
//! What's signed ([`message`]) is a fixed label, the scope, the request's method and path
//! (request scope only), the time, the nonce, and the SHA-256 of the session token. Two scopes:
//!
//! - **[`Scope::Request`]**: one request, its method and path included; accepted within
//!   [`REQUEST_WINDOW_MILLIS`] of the server's clock and **only once** (the server remembers
//!   nonces). For the app's own requests: files now, sync later.
//! - **[`Scope::Session`]**: no method or path; accepted within [`SESSION_WINDOW_MILLIS`] and
//!   reusable inside it. Dioxus 0.7 sends server-function calls with one shared set of headers
//!   and no per-request hook, so those calls carry a session signature, renewed every
//!   [`SESSION_RENEW_MILLIS`]. It still proves the device holds the key; a captured one works
//!   only for a minute or two (and capturing it means breaking HTTPS).
//!
//! The desktop signs with the **server's** time (its clock plus the offset the handshake
//! showed), so a computer whose clock is wrong still works.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::time::UnixMillis;

/// The request header carrying the signature.
pub const HEADER: &str = "x-device-signature";

/// The format's version, the header's first field.
const VERSION: &str = "v1";

/// What every signed message starts with, so a signature made for something else can't pass.
const LABEL: &str = "vehiclemanager-device-signature-v1";

/// How far from the server's clock a request signature may be: 5 minutes.
pub const REQUEST_WINDOW_MILLIS: i64 = 5 * 60 * 1000;

/// How far from the server's clock a session signature may be: 2 minutes.
pub const SESSION_WINDOW_MILLIS: i64 = 2 * 60 * 1000;

/// How often the desktop renews its session signature: every 30 seconds.
pub const SESSION_RENEW_MILLIS: i64 = 30 * 1000;

// A session signature is renewed well before it lapses, even with a little clock drift.
const _: () = assert!(SESSION_RENEW_MILLIS * 2 < SESSION_WINDOW_MILLIS);

/// What a signature covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Any request of this session, briefly (see the module docs).
    Session,
    /// One request: its method and path, once.
    Request,
}

impl Scope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Request => "request",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Self::Session, Self::Request]
            .into_iter()
            .find(|scope| scope.as_str() == text)
    }

    /// How far from the server's clock its signatures may be.
    #[must_use]
    pub const fn window_millis(self) -> i64 {
        match self {
            Self::Session => SESSION_WINDOW_MILLIS,
            Self::Request => REQUEST_WINDOW_MILLIS,
        }
    }
}

/// A signature, as the header carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceSignature {
    /// What it covers.
    pub scope: Scope,
    /// When it was made, on the server's clock.
    pub time: UnixMillis,
    /// A random value, so no two are alike.
    pub nonce: [u8; 16],
    /// The Ed25519 signature over [`message`].
    pub signature: [u8; 64],
}

/// Why a request's signature was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureError {
    /// No signature, but the session's device needs one.
    Missing,
    /// The header isn't in this format.
    Malformed,
    /// Its time is too far from the server's clock.
    OutOfTime,
    /// A request signature already used.
    Reused,
    /// The signature doesn't match the device's key and this request.
    Wrong,
}

impl std::fmt::Display for SignatureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "no device signature",
            Self::Malformed => "malformed device signature",
            Self::OutOfTime => "device signature out of time",
            Self::Reused => "device signature already used",
            Self::Wrong => "device signature doesn't match",
        })
    }
}

impl std::error::Error for SignatureError {}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0; N];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *byte = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(bytes)
}

/// The SHA-256 of a session token as the client holds it (64 hex characters), as the server
/// stores it; `None` if the token isn't in that form.
#[must_use]
pub fn token_hash(token: &str) -> Option<[u8; 32]> {
    let bytes: [u8; 32] = from_hex(token)?;
    Some(Sha256::digest(bytes).into())
}

/// The bytes a signature covers. For [`Scope::Session`], `method` and `path` are left out.
#[must_use]
pub fn message(
    scope: Scope,
    method: &str,
    path_and_query: &str,
    time: UnixMillis,
    nonce: &[u8; 16],
    token_hash: &[u8; 32],
) -> Vec<u8> {
    let (method, path) = match scope {
        Scope::Session => ("", ""),
        Scope::Request => (method, path_and_query),
    };
    format!(
        "{LABEL}\n{}\n{}\n{}\n{}\n{}\n{}",
        scope.as_str(),
        method.to_ascii_uppercase(),
        path,
        time.as_millis(),
        to_hex(nonce),
        to_hex(token_hash),
    )
    .into_bytes()
}

impl DeviceSignature {
    /// Signs a request (or, for [`Scope::Session`], the session) with the device's secret key
    /// (its 32-byte seed).
    #[must_use]
    pub fn sign(
        secret_key: &[u8; 32],
        scope: Scope,
        method: &str,
        path_and_query: &str,
        time: UnixMillis,
        nonce: [u8; 16],
        token_hash: &[u8; 32],
    ) -> Self {
        let key = SigningKey::from_bytes(secret_key);
        let signed = message(scope, method, path_and_query, time, &nonce, token_hash);
        Self {
            scope,
            time,
            nonce,
            signature: key.sign(&signed).to_bytes(),
        }
    }

    /// The header's value.
    #[must_use]
    pub fn to_header(&self) -> String {
        format!(
            "{VERSION};scope={};t={};n={};s={}",
            self.scope.as_str(),
            self.time.as_millis(),
            to_hex(&self.nonce),
            to_hex(&self.signature)
        )
    }

    /// A header's value read back.
    ///
    /// # Errors
    ///
    /// [`SignatureError::Malformed`] if it isn't in this format.
    pub fn parse(header: &str) -> Result<Self, SignatureError> {
        let mut fields = header.trim().split(';');
        if fields.next() != Some(VERSION) {
            return Err(SignatureError::Malformed);
        }
        let mut field = |name: &str| {
            fields
                .next()
                .and_then(|field| field.strip_prefix(name)?.strip_prefix('='))
                .ok_or(SignatureError::Malformed)
        };
        let scope = Scope::parse(field("scope")?).ok_or(SignatureError::Malformed)?;
        let time = field("t")?
            .parse::<i64>()
            .map_err(|_| SignatureError::Malformed)?;
        let nonce = from_hex(field("n")?).ok_or(SignatureError::Malformed)?;
        let signature = from_hex(field("s")?).ok_or(SignatureError::Malformed)?;
        if fields.next().is_some() {
            return Err(SignatureError::Malformed);
        }
        Ok(Self {
            scope,
            time: UnixMillis::from_millis(time),
            nonce,
            signature,
        })
    }

    /// Whether this signature, made by the device with `public_key`, covers this request of
    /// the session whose token hashes to `token_hash`, at `now` on the server's clock. It
    /// doesn't check reuse: the server remembers request nonces itself.
    ///
    /// # Errors
    ///
    /// [`SignatureError::OutOfTime`] or [`SignatureError::Wrong`].
    pub fn verify(
        &self,
        public_key: &[u8; 32],
        method: &str,
        path_and_query: &str,
        token_hash: &[u8; 32],
        now: UnixMillis,
    ) -> Result<(), SignatureError> {
        let distance = now.as_millis().abs_diff(self.time.as_millis());
        if distance > self.scope.window_millis().unsigned_abs() {
            return Err(SignatureError::OutOfTime);
        }
        let key = VerifyingKey::from_bytes(public_key).map_err(|_| SignatureError::Wrong)?;
        let signed = message(
            self.scope,
            method,
            path_and_query,
            self.time,
            &self.nonce,
            token_hash,
        );
        key.verify_strict(&signed, &Signature::from_bytes(&self.signature))
            .map_err(|_| SignatureError::Wrong)
    }
}

/// The public key of the device whose secret key (seed) is `secret_key`.
#[must_use]
pub fn public_key_of(secret_key: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(secret_key)
        .verifying_key()
        .to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 32] = [7; 32];
    const OTHER: [u8; 32] = [8; 32];
    const NOW: UnixMillis = UnixMillis::from_millis(1_762_700_000_000);
    const TOKEN: [u8; 32] = [3; 32];

    fn signed(scope: Scope) -> DeviceSignature {
        DeviceSignature::sign(
            &SECRET,
            scope,
            "post",
            "/api/v1/events/edit",
            NOW,
            [5; 16],
            &TOKEN,
        )
    }

    fn check(
        signature: &DeviceSignature,
        path: &str,
        token: &[u8; 32],
        now: UnixMillis,
    ) -> Result<(), SignatureError> {
        signature.verify(&public_key_of(&SECRET), "POST", path, token, now)
    }

    #[test]
    fn a_request_signature_passes_for_its_own_request_only() {
        let signature = signed(Scope::Request);
        assert_eq!(
            check(&signature, "/api/v1/events/edit", &TOKEN, NOW),
            Ok(())
        );
        assert_eq!(
            check(&signature, "/api/v1/users/reset", &TOKEN, NOW),
            Err(SignatureError::Wrong),
            "another path"
        );
        assert_eq!(
            signature.verify(
                &public_key_of(&SECRET),
                "GET",
                "/api/v1/events/edit",
                &TOKEN,
                NOW
            ),
            Err(SignatureError::Wrong),
            "another method"
        );
        assert_eq!(
            check(&signature, "/api/v1/events/edit", &[4; 32], NOW),
            Err(SignatureError::Wrong),
            "another session's token"
        );
        assert_eq!(
            signature.verify(
                &public_key_of(&OTHER),
                "POST",
                "/api/v1/events/edit",
                &TOKEN,
                NOW
            ),
            Err(SignatureError::Wrong),
            "another device's key"
        );
    }

    #[test]
    fn a_session_signature_covers_any_path_of_its_session() {
        let signature = signed(Scope::Session);
        assert_eq!(
            check(&signature, "/api/v1/users/reset", &TOKEN, NOW),
            Ok(())
        );
        assert_eq!(
            check(&signature, "/api/v1/users/reset", &[4; 32], NOW),
            Err(SignatureError::Wrong)
        );
    }

    #[test]
    fn each_scope_has_its_time_window() {
        let at = |offset: i64| UnixMillis::from_millis(NOW.as_millis() + offset);
        let request = signed(Scope::Request);
        let session = signed(Scope::Session);
        let path = "/api/v1/events/edit";
        assert_eq!(
            check(&request, path, &TOKEN, at(REQUEST_WINDOW_MILLIS)),
            Ok(())
        );
        assert_eq!(
            check(&request, path, &TOKEN, at(-REQUEST_WINDOW_MILLIS)),
            Ok(())
        );
        assert_eq!(
            check(&request, path, &TOKEN, at(REQUEST_WINDOW_MILLIS + 1)),
            Err(SignatureError::OutOfTime)
        );
        assert_eq!(
            check(&session, path, &TOKEN, at(SESSION_WINDOW_MILLIS)),
            Ok(())
        );
        assert_eq!(
            check(&session, path, &TOKEN, at(SESSION_WINDOW_MILLIS + 1)),
            Err(SignatureError::OutOfTime)
        );
    }

    #[test]
    fn the_header_round_trips_and_anything_else_is_malformed() {
        let signature = signed(Scope::Request);
        let header = signature.to_header();
        assert!(
            header.starts_with("v1;scope=request;t=1762700000000;n="),
            "{header}"
        );
        assert_eq!(DeviceSignature::parse(&header), Ok(signature));
        for bad in [
            "",
            "v2;scope=request;t=1;n=00;s=00",
            &header.replace("scope=request", "scope=forever"),
            &header.replace(";t=", ";time="),
            &format!("{header};extra=1"),
            &header[..header.len() - 2],
        ] {
            assert_eq!(
                DeviceSignature::parse(bad),
                Err(SignatureError::Malformed),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_changed_signature_byte_fails() {
        let mut signature = signed(Scope::Request);
        signature.signature[10] ^= 1;
        assert_eq!(
            check(&signature, "/api/v1/events/edit", &TOKEN, NOW),
            Err(SignatureError::Wrong)
        );
    }

    #[test]
    fn the_token_hash_matches_the_servers() {
        let token = "ab".repeat(32);
        let expected: [u8; 32] = Sha256::digest([0xab; 32]).into();
        assert_eq!(token_hash(&token), Some(expected));
        assert_eq!(token_hash("short"), None);
        assert_eq!(token_hash(&"zz".repeat(32)), None);
    }
}
