//! The address of the server a native app talks to, and the rules it must follow.
//!
//! A server address is a scheme, a host, and an optional port, nothing else:
//! `https://nas.example.ts.net` or `https://nas.example.ts.net:8443`. Server functions
//! add their own `/api/...` paths to it.
//!
//! **`https://` is required**, except `http://` to this computer (a loopback host such as
//! `localhost` or `127.0.0.1`) for local development. The session token travels with every
//! request, so it must never cross a network unencrypted (decision #027).
//!
//! # Examples
//!
//! ```
//! use domain::server_address::{ServerAddress, ServerAddressError};
//!
//! let nas = ServerAddress::parse(" https://NAS.example.ts.net/ ").unwrap();
//! assert_eq!(nas.as_str(), "https://nas.example.ts.net");
//!
//! assert!(ServerAddress::parse("http://localhost:8080").is_ok());
//! assert_eq!(
//!     ServerAddress::parse("http://nas.example.ts.net"),
//!     Err(ServerAddressError::InsecureRemote)
//! );
//! ```

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::i18n::{MessageId, messages};

/// A server address that follows the rules above, normalized: lowercase scheme and
/// host, no trailing `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddress(String);

/// Why an address was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerAddressError {
    /// Nothing was entered.
    Empty,
    /// It doesn't start with `https://` or `http://`.
    UnsupportedScheme,
    /// `http://` to a host other than this computer.
    InsecureRemote,
    /// Nothing between the scheme and the port.
    MissingHost,
    /// The host has characters a host name can't have, or an empty label (`a..b`).
    InvalidHost,
    /// The port isn't a number from 1 to 65535.
    InvalidPort,
    /// A path, query, fragment, or login (`user@`) after the host.
    ExtraParts,
}

impl ServerAddressError {
    /// Every reason, for exhaustive tests.
    pub const ALL: [Self; 7] = [
        Self::Empty,
        Self::UnsupportedScheme,
        Self::InsecureRemote,
        Self::MissingHost,
        Self::InvalidHost,
        Self::InvalidPort,
        Self::ExtraParts,
    ];

    /// The message explaining the problem to the user.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Empty => messages::SERVER_ADDRESS_EMPTY,
            Self::UnsupportedScheme => messages::SERVER_ADDRESS_UNSUPPORTED_SCHEME,
            Self::InsecureRemote => messages::SERVER_ADDRESS_INSECURE_REMOTE,
            Self::MissingHost => messages::SERVER_ADDRESS_MISSING_HOST,
            Self::InvalidHost => messages::SERVER_ADDRESS_INVALID_HOST,
            Self::InvalidPort => messages::SERVER_ADDRESS_INVALID_PORT,
            Self::ExtraParts => messages::SERVER_ADDRESS_EXTRA_PARTS,
        }
    }
}

/// The schemes an address may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    Https,
    Http,
}

impl ServerAddress {
    /// Checks `input` against the rules and normalizes it. Surrounding spaces and one
    /// trailing `/` are ignored.
    ///
    /// # Errors
    ///
    /// Returns the first rule the address breaks; see [`ServerAddressError`].
    pub fn parse(input: &str) -> Result<Self, ServerAddressError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ServerAddressError::Empty);
        }
        let (scheme, rest) = split_scheme(trimmed)?;
        let authority = rest.strip_suffix('/').unwrap_or(rest);
        if authority.contains(['/', '?', '#', '@']) {
            return Err(ServerAddressError::ExtraParts);
        }

        let (host, port) = split_host_and_port(authority)?;
        let host = host.to_ascii_lowercase();
        check_host(&host)?;
        if scheme == Scheme::Http && !is_loopback(&host) {
            return Err(ServerAddressError::InsecureRemote);
        }

        let scheme_text = match scheme {
            Scheme::Https => "https",
            Scheme::Http => "http",
        };
        let port_text = port.map(|port| format!(":{port}")).unwrap_or_default();
        Ok(Self(format!("{scheme_text}://{host}{port_text}")))
    }

    /// The normalized address, e.g. `https://nas.example.ts.net`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Splits off the scheme, case-insensitively.
fn split_scheme(address: &str) -> Result<(Scheme, &str), ServerAddressError> {
    let (scheme, rest) = address
        .split_once("://")
        .ok_or(ServerAddressError::UnsupportedScheme)?;
    if scheme.eq_ignore_ascii_case("https") {
        Ok((Scheme::Https, rest))
    } else if scheme.eq_ignore_ascii_case("http") {
        Ok((Scheme::Http, rest))
    } else {
        Err(ServerAddressError::UnsupportedScheme)
    }
}

/// Splits `host[:port]` or `[ipv6][:port]` and checks the port.
fn split_host_and_port(authority: &str) -> Result<(&str, Option<u16>), ServerAddressError> {
    let (host, port_text) = if let Some(after_bracket) = authority.strip_prefix('[') {
        // IPv6 literal: the host keeps its brackets, as it does in a URL.
        let (inside, after) = after_bracket
            .split_once(']')
            .ok_or(ServerAddressError::InvalidHost)?;
        let host = &authority[..inside.len() + 2];
        match after {
            "" => (host, None),
            _ => (
                host,
                Some(
                    after
                        .strip_prefix(':')
                        .ok_or(ServerAddressError::InvalidHost)?,
                ),
            ),
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };

    let port = port_text.map(parse_port).transpose()?;
    Ok((host, port))
}

/// A port from 1 to 65535, digits only.
fn parse_port(text: &str) -> Result<u16, ServerAddressError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ServerAddressError::InvalidPort);
    }
    match text.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(ServerAddressError::InvalidPort),
    }
}

/// A host name (letters, digits, `-`, in dot-separated labels), an IPv4 address, or a
/// bracketed IPv6 address. `host` is already lowercase.
fn check_host(host: &str) -> Result<(), ServerAddressError> {
    if host.is_empty() {
        return Err(ServerAddressError::MissingHost);
    }
    if let Some(inside) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inside
            .parse::<Ipv6Addr>()
            .map(|_| ())
            .map_err(|_| ServerAddressError::InvalidHost);
    }
    let valid_label = |label: &str| {
        !label.is_empty()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    if host.split('.').all(valid_label) {
        Ok(())
    } else {
        Err(ServerAddressError::InvalidHost)
    }
}

/// Whether `host` is this computer: `localhost`, `127.0.0.0/8`, or `[::1]`.
fn is_loopback(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    if let Some(inside) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inside
            .parse::<Ipv6Addr>()
            .is_ok_and(|address| address.is_loopback());
    }
    host.parse::<Ipv4Addr>()
        .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(input: &str) -> Result<String, ServerAddressError> {
        ServerAddress::parse(input).map(|address| address.as_str().to_owned())
    }

    #[test]
    fn https_addresses_are_accepted_and_normalized() {
        assert_eq!(
            parsed("https://fenec.tail1.ts.net"),
            Ok("https://fenec.tail1.ts.net".into())
        );
        assert_eq!(
            parsed("  HTTPS://Fenec.Tail1.TS.net/  "),
            Ok("https://fenec.tail1.ts.net".into())
        );
        assert_eq!(
            parsed("https://fenec.tail1.ts.net:8443"),
            Ok("https://fenec.tail1.ts.net:8443".into())
        );
        assert_eq!(
            parsed("https://100.64.0.1"),
            Ok("https://100.64.0.1".into())
        );
        assert_eq!(
            parsed("https://[fd7a::1]:443"),
            Ok("https://[fd7a::1]:443".into())
        );
    }

    #[test]
    fn http_is_accepted_only_for_this_computer() {
        assert_eq!(
            parsed("http://localhost:8080"),
            Ok("http://localhost:8080".into())
        );
        assert_eq!(
            parsed("http://127.0.0.1:8080"),
            Ok("http://127.0.0.1:8080".into())
        );
        assert_eq!(parsed("http://127.1.2.3"), Ok("http://127.1.2.3".into()));
        assert_eq!(parsed("http://[::1]:8080"), Ok("http://[::1]:8080".into()));

        for remote in [
            "http://fenec.tail1.ts.net",
            "http://100.64.0.1:8410",
            "http://192.168.1.10:8410",
            "http://[fd7a::1]",
        ] {
            assert_eq!(
                parsed(remote),
                Err(ServerAddressError::InsecureRemote),
                "{remote}"
            );
        }
    }

    #[test]
    fn addresses_breaking_a_rule_are_refused_with_that_rule() {
        let cases = [
            ("", ServerAddressError::Empty),
            ("   ", ServerAddressError::Empty),
            ("fenec.tail1.ts.net", ServerAddressError::UnsupportedScheme),
            (
                "ftp://fenec.tail1.ts.net",
                ServerAddressError::UnsupportedScheme,
            ),
            ("https://", ServerAddressError::MissingHost),
            ("https://:8443", ServerAddressError::MissingHost),
            ("https://fenec..ts.net", ServerAddressError::InvalidHost),
            ("https://fenec_nas.ts.net", ServerAddressError::InvalidHost),
            ("https://-fenec.ts.net", ServerAddressError::InvalidHost),
            ("https://[not-ipv6]", ServerAddressError::InvalidHost),
            ("https://[::1", ServerAddressError::InvalidHost),
            ("https://fenec.ts.net:", ServerAddressError::InvalidPort),
            ("https://fenec.ts.net:0", ServerAddressError::InvalidPort),
            (
                "https://fenec.ts.net:65536",
                ServerAddressError::InvalidPort,
            ),
            ("https://fenec.ts.net:+443", ServerAddressError::InvalidPort),
            ("https://fenec.ts.net/app", ServerAddressError::ExtraParts),
            ("https://fenec.ts.net?x=1", ServerAddressError::ExtraParts),
            ("https://fenec.ts.net#top", ServerAddressError::ExtraParts),
            ("https://me@fenec.ts.net", ServerAddressError::ExtraParts),
        ];
        for (input, expected) in cases {
            assert_eq!(parsed(input), Err(expected), "{input:?}");
        }
    }

    #[test]
    fn every_address_error_has_an_english_message() {
        let english = crate::i18n::Translator::english();
        for error in ServerAddressError::ALL {
            assert!(english.has(error.message()), "{error:?}");
        }
    }
}
