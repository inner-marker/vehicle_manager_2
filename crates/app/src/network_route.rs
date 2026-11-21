//! Whether this computer has no network at all, asked before the session check so a desktop
//! with no network opens its copy at once instead of waiting `SESSION_WAIT` (phase-6.md 1f).
//!
//! Asking sends nothing: a UDP socket "connects" to an address, which only asks the system
//! for a route to it. The computer counts as having **no network** when:
//! - it has no route to the internet (to a public IPv4 or IPv6 address), **and**
//! - the server can't be reached without it: its name can't be looked up, it has no route,
//!   or every address it has is a Tailscale one (whose tunnel keeps its routes with Wi-Fi off,
//!   but needs the internet underneath).
//!
//! Anything else (Wi-Fi on but the server down, Tailscale turned off, a server on a home
//! network with no internet) waits as before. A wrong "no network" only opens the copy a
//! little early; the app keeps trying the server, as it does after any wait.
//!
//! Only the desktop asks (`has_no_network`); the rest builds everywhere but the browser, so CI
//! tests the route check on macOS, Windows, and Linux.

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
use std::net::ToSocketAddrs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
use std::time::Duration;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
use domain::log_area;

/// Public addresses to ask for a route to (Cloudflare's DNS). Nothing is sent to them.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
const PUBLIC_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
const PUBLIC_V6: IpAddr = IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111));

/// The longest the server's name may take to look up before it counts as unknown.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
const LOOKUP_WAIT: Duration = Duration::from_secs(1);

/// The host and port in a server address such as `https://nas.example.ts.net:8443/`, with
/// the scheme's port (443 or 80) when none is given. `None` if it isn't `http` or `https`.
#[must_use]
pub fn host_and_port(address: &str) -> Option<(String, u16)> {
    let (scheme, rest) = address.split_once("://")?;
    let default_port = match scheme.to_ascii_lowercase().as_str() {
        "https" => 443,
        "http" => 80,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        // An IPv6 address: [::1]:8443
        let (host, after) = bracketed.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None if after.is_empty() => default_port,
            None => return None,
        };
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().ok()?),
            None => (authority, default_port),
        }
    };
    if host.is_empty() {
        return None;
    }
    Some((host.to_owned(), port))
}

/// Whether `ip` is a Tailscale address (100.64.0.0/10, or `fd7a:115c:a1e0::/48`).
#[must_use]
pub fn is_tailscale(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [first, second, ..] = v4.octets();
            first == 100 && (64..128).contains(&second)
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
        }
    }
}

/// The verdict, from what the system said: whether the internet has a route, and the server's
/// addresses with whether each has one (`None`: its name couldn't be looked up in time).
#[must_use]
pub fn no_network(internet_routable: bool, server: Option<&[(IpAddr, bool)]>) -> bool {
    if internet_routable {
        return false;
    }
    match server {
        None => true,
        Some(addresses) => addresses
            .iter()
            .all(|&(ip, routable)| !routable || is_tailscale(ip)),
    }
}

/// Whether the system has a route to `ip` (nothing is sent).
#[must_use]
pub fn routable(ip: IpAddr, port: u16) -> bool {
    let local: SocketAddr = match ip {
        IpAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        IpAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    UdpSocket::bind(local)
        .and_then(|socket| socket.connect(SocketAddr::new(ip, port)))
        .is_ok()
}

/// Whether this computer has no network at all for the server at `address` (see the module
/// notes). Takes at most about a second; `false` whenever it can't tell.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub async fn has_no_network(address: &str) -> bool {
    let internet = routable(PUBLIC_V4, 53) || routable(PUBLIC_V6, 53);
    if internet {
        return false;
    }
    let Some((host, port)) = host_and_port(address) else {
        return false;
    };
    let lookup = tokio::task::spawn_blocking(move || {
        (host.as_str(), port).to_socket_addrs().map(|addresses| {
            addresses
                .map(|socket| (socket.ip(), routable(socket.ip(), socket.port())))
                .collect::<Vec<_>>()
        })
    });
    let server = match tokio::time::timeout(LOOKUP_WAIT, lookup).await {
        Ok(Ok(Ok(addresses))) => Some(addresses),
        Ok(Ok(Err(_)) | Err(_)) | Err(_) => None,
    };
    let verdict = no_network(false, server.as_deref());
    if verdict {
        tracing::info!(target: log_area::SYNC, "this computer has no network; not waiting for the server");
    }
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_and_port_reads_server_addresses() {
        assert_eq!(
            host_and_port("https://nas.example.ts.net"),
            Some(("nas.example.ts.net".into(), 443))
        );
        assert_eq!(
            host_and_port("https://nas.example.ts.net:8443/"),
            Some(("nas.example.ts.net".into(), 8443))
        );
        assert_eq!(
            host_and_port("http://192.168.1.20:8410/api?x=1"),
            Some(("192.168.1.20".into(), 8410))
        );
        assert_eq!(
            host_and_port("http://[::1]:8080"),
            Some(("::1".into(), 8080))
        );
        assert_eq!(
            host_and_port("HTTPS://[fd7a:115c:a1e0::1]"),
            Some(("fd7a:115c:a1e0::1".into(), 443))
        );
        assert_eq!(host_and_port("ftp://nas"), None);
        assert_eq!(host_and_port("nas:8443"), None);
        assert_eq!(host_and_port("https://:8443"), None);
        assert_eq!(host_and_port("https://nas:port"), None);
    }

    #[test]
    fn tailscale_addresses_are_recognised() {
        assert!(is_tailscale("100.64.0.1".parse().unwrap()));
        assert!(is_tailscale("100.127.255.254".parse().unwrap()));
        assert!(is_tailscale("fd7a:115c:a1e0::5".parse().unwrap()));
        assert!(!is_tailscale("100.128.0.1".parse().unwrap()));
        assert!(!is_tailscale("100.63.0.1".parse().unwrap()));
        assert!(!is_tailscale("192.168.1.20".parse().unwrap()));
        assert!(!is_tailscale("fd00::1".parse().unwrap()));
    }

    #[test]
    fn no_network_only_without_the_internet_and_without_a_way_to_the_server() {
        let tailnet: IpAddr = "100.100.1.2".parse().unwrap();
        let home: IpAddr = "192.168.1.20".parse().unwrap();
        // The internet has a route: always wait as before.
        assert!(!no_network(true, None));
        assert!(!no_network(true, Some(&[(home, false)])));
        // No internet, and the server's name can't be looked up, or it has no route.
        assert!(no_network(false, None));
        assert!(no_network(false, Some(&[])));
        assert!(no_network(false, Some(&[(home, false)])));
        // No internet, and the server is only reached through Tailscale.
        assert!(no_network(false, Some(&[(tailnet, true)])));
        // No internet, but the server is on the home network: wait as before.
        assert!(!no_network(false, Some(&[(home, true)])));
        assert!(!no_network(false, Some(&[(tailnet, true), (home, true)])));
    }

    #[test]
    fn this_computer_has_a_route_to_itself() {
        // Every system (macOS, Windows, and Linux in CI) routes to loopback.
        assert!(routable(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080));
    }
}
