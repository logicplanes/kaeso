use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use ipnet::IpNet;

use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientIp(pub Option<IpAddr>);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        let forwarded = parts
            .headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join(",");
        Ok(ClientIp(resolve(
            peer,
            &forwarded,
            &state.config.trusted_proxies,
        )))
    }
}

// A dual-stack listener reports an IPv4 peer as ::ffff:a.b.c.d, which no IPv4 network of the
// trusted proxies contains and which would count apart from the same client's other requests.
pub fn resolve(peer: Option<IpAddr>, forwarded: &str, trusted: &[IpNet]) -> Option<IpAddr> {
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|n| n.contains(ip));
    let peer = peer?.to_canonical();
    if !is_trusted(&peer) {
        return Some(peer);
    }
    let mut client = peer;
    for hop in forwarded
        .rsplit(',')
        .map(str::trim)
        .filter(|h| !h.is_empty())
    {
        let Ok(ip) = hop.parse::<IpAddr>().map(|ip| ip.to_canonical()) else {
            break;
        };
        client = ip;
        if !is_trusted(&ip) {
            break;
        }
    }
    Some(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets() -> Vec<IpNet> {
        vec![
            "10.0.0.0/8".parse().unwrap(),
            "127.0.0.1/32".parse().unwrap(),
        ]
    }

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn untrusted_peers_cannot_choose_their_address() {
        assert_eq!(
            resolve(ip("203.0.113.9"), "1.2.3.4", &nets()),
            ip("203.0.113.9")
        );
    }

    #[test]
    fn trusted_proxies_are_skipped_from_the_right() {
        assert_eq!(
            resolve(ip("10.0.0.2"), "198.51.100.7", &nets()),
            ip("198.51.100.7")
        );
        assert_eq!(
            resolve(ip("10.0.0.2"), "6.6.6.6, 198.51.100.7, 10.0.0.3", &nets()),
            ip("198.51.100.7")
        );
        assert_eq!(resolve(ip("10.0.0.2"), "", &nets()), ip("10.0.0.2"));
        assert_eq!(
            resolve(ip("10.0.0.2"), "x, 10.0.0.9", &nets()),
            ip("10.0.0.9")
        );
        assert_eq!(resolve(None, "1.2.3.4", &nets()), None);
    }

    #[test]
    fn ipv4_mapped_addresses_are_the_ipv4_address() {
        assert_eq!(
            resolve(ip("::ffff:10.0.0.2"), "::ffff:198.51.100.7", &nets()),
            ip("198.51.100.7"),
            "a trusted proxy on a dual-stack socket is still trusted"
        );
        assert_eq!(
            resolve(ip("::ffff:203.0.113.9"), "1.2.3.4", &nets()),
            ip("203.0.113.9")
        );
        assert_eq!(
            resolve(
                ip("10.0.0.2"),
                "::ffff:10.0.0.3, ::ffff:198.51.100.7, 10.0.0.4",
                &nets()
            ),
            ip("198.51.100.7")
        );
        assert_eq!(resolve(ip("2001:db8::1"), "", &nets()), ip("2001:db8::1"));
    }
}
