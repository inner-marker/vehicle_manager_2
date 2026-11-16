//! Who sent a request, for the sign-in limit (decision #071; `docs/sub-plans/04-security.md`
//! §5.1).
//!
//! The server sits behind one or more proxies (`tailscale serve`, and maybe Caddy), and never
//! sees the client's own address. Each proxy **adds** the address it heard from to the end of
//! `X-Forwarded-For`, so the entry [`ProxyHops`] places from the right was written by the
//! outermost proxy we trust. Entries further left came from the client and can say anything,
//! so they're never used.

use std::net::IpAddr;
use std::sync::OnceLock;

/// The environment variable for the number of proxies, without the app prefix.
pub const PROXY_HOPS_ENV_NAME: &str = "PROXY_HOPS";

/// The key for a request that didn't come through the expected proxies (or whose entry isn't
/// an address). Only something on the server's own machine can reach it that way.
pub const DIRECT: &str = "direct";

/// How many proxies sit in front of the server: 1 (`tailscale serve`) by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyHops(u8);

/// The most proxies allowed: more is surely a typo.
const MAX_HOPS: u8 = 5;

impl Default for ProxyHops {
    fn default() -> Self {
        Self(1)
    }
}

/// Why a proxy count was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyHopsError(String);

impl std::fmt::Display for ProxyHopsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "expected a whole number from 0 to {MAX_HOPS}, got {:?}",
            self.0
        )
    }
}

impl std::error::Error for ProxyHopsError {}

impl ProxyHops {
    /// The count from its setting: empty means the default (1).
    ///
    /// # Errors
    ///
    /// [`ProxyHopsError`] for anything but a whole number from 0 to 5.
    pub fn parse(setting: &str) -> Result<Self, ProxyHopsError> {
        let setting = setting.trim();
        if setting.is_empty() {
            return Ok(Self::default());
        }
        setting
            .parse::<u8>()
            .ok()
            .filter(|hops| *hops <= MAX_HOPS)
            .map(Self)
            .ok_or_else(|| ProxyHopsError(setting.to_owned()))
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The client named by `forwarded_for` (an `X-Forwarded-For` value, if any) behind `hops`
/// proxies: its address, or [`DIRECT`].
#[must_use]
pub fn client_from_forwarded(forwarded_for: Option<&str>, hops: ProxyHops) -> String {
    let hops = usize::from(hops.get());
    if hops == 0 {
        return DIRECT.to_owned();
    }
    let entries: Vec<&str> = forwarded_for
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    entries
        .len()
        .checked_sub(hops)
        .and_then(|index| entries.get(index))
        .and_then(|entry| entry.parse::<IpAddr>().ok())
        .map_or_else(|| DIRECT.to_owned(), |address| address.to_string())
}

/// This server's proxy count, set once at startup.
static PROXY_HOPS: OnceLock<ProxyHops> = OnceLock::new();

/// Sets this server's proxy count, once, at startup (later calls keep the first).
pub fn set_proxy_hops(hops: ProxyHops) {
    let _ = PROXY_HOPS.set(hops);
}

/// This server's proxy count (the default until set).
#[must_use]
pub fn proxy_hops() -> ProxyHops {
    PROXY_HOPS.get().copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hops(count: u8) -> ProxyHops {
        ProxyHops(count)
    }

    #[test]
    fn the_count_parses_with_one_by_default() {
        assert_eq!(ProxyHops::parse(""), Ok(hops(1)));
        assert_eq!(ProxyHops::parse(" 2 "), Ok(hops(2)));
        assert_eq!(ProxyHops::parse("0"), Ok(hops(0)));
        assert!(ProxyHops::parse("6").is_err());
        assert!(ProxyHops::parse("-1").is_err());
        assert!(ProxyHops::parse("one").is_err());
    }

    #[test]
    fn behind_one_proxy_the_right_most_entry_is_the_client() {
        assert_eq!(
            client_from_forwarded(Some("100.64.0.7"), hops(1)),
            "100.64.0.7"
        );
    }

    #[test]
    fn an_entry_the_client_wrote_itself_is_ignored() {
        // The client sent "1.2.3.4"; tailscale serve added the real address after it.
        assert_eq!(
            client_from_forwarded(Some("1.2.3.4, 100.64.0.7"), hops(1)),
            "100.64.0.7"
        );
    }

    #[test]
    fn behind_two_proxies_the_second_from_the_right_is_the_client() {
        // Caddy added the client; tailscale serve (inside) added Caddy's address.
        assert_eq!(
            client_from_forwarded(Some("9.9.9.9, 203.0.113.5, 100.64.0.1"), hops(2)),
            "203.0.113.5"
        );
    }

    #[test]
    fn too_few_entries_a_missing_header_or_no_proxies_is_direct() {
        assert_eq!(client_from_forwarded(Some("100.64.0.7"), hops(2)), DIRECT);
        assert_eq!(client_from_forwarded(None, hops(1)), DIRECT);
        assert_eq!(client_from_forwarded(Some(" , "), hops(1)), DIRECT);
        assert_eq!(client_from_forwarded(Some("100.64.0.7"), hops(0)), DIRECT);
    }

    #[test]
    fn an_entry_that_is_not_an_address_is_direct_and_ipv6_is_kept() {
        assert_eq!(client_from_forwarded(Some("unknown"), hops(1)), DIRECT);
        assert_eq!(
            client_from_forwarded(Some("fd7a:115c:a1e0::1"), hops(1)),
            "fd7a:115c:a1e0::1"
        );
    }
}
