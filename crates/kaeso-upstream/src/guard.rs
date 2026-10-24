use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use ipnet::IpNet;
use kaeso_core::net::{is_link_local_or_metadata, is_public};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Private {
    None,
    Every,
    Networks(Vec<IpNet>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    public: bool,
    private: Private,
}

impl Default for Reach {
    fn default() -> Self {
        Reach::public_only()
    }
}

impl Reach {
    pub fn public_only() -> Reach {
        Reach {
            public: true,
            private: Private::None,
        }
    }

    pub fn every_private_network() -> Reach {
        Reach {
            public: true,
            private: Private::Every,
        }
    }

    pub fn networks(allowed: Vec<IpNet>) -> Reach {
        Reach {
            public: true,
            private: if allowed.is_empty() {
                Private::None
            } else {
                Private::Networks(allowed)
            },
        }
    }

    #[must_use]
    pub fn without_public(&self) -> Reach {
        Reach {
            public: false,
            private: self.private.clone(),
        }
    }

    // Link-local and cloud metadata addresses stay refused even when an allowlist
    // names them: they answer with the host's own credentials.
    pub fn permits(&self, ip: IpAddr) -> bool {
        if is_link_local_or_metadata(ip) {
            return false;
        }
        if is_public(ip) {
            return self.public;
        }
        match &self.private {
            Private::None => false,
            Private::Every => true,
            Private::Networks(nets) => nets.iter().any(|n| n.contains(&ip)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{host} resolves only to addresses this install does not allow")]
pub struct Refused {
    pub host: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("cannot resolve {host}: {source}")]
    Lookup {
        host: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Refused(#[from] Refused),
}

type Found = Pin<Box<dyn Future<Output = std::io::Result<Vec<SocketAddr>>> + Send>>;

#[derive(Clone)]
pub struct Lookup(Arc<dyn Fn(String, u16) -> Found + Send + Sync>);

impl std::fmt::Debug for Lookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Lookup")
    }
}

impl Default for Lookup {
    fn default() -> Self {
        Lookup::new(|host, port| {
            Box::pin(async move { Ok(tokio::net::lookup_host((host, port)).await?.collect()) })
        })
    }
}

impl Lookup {
    pub fn new(f: impl Fn(String, u16) -> Found + Send + Sync + 'static) -> Lookup {
        Lookup(Arc::new(f))
    }

    async fn find(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        (self.0)(host.to_owned(), port).await
    }
}

fn bare(host: &str) -> &str {
    host.trim_start_matches('[').trim_end_matches(']')
}

pub async fn resolve(
    host: &str,
    port: u16,
    reach: &Reach,
    lookup: &Lookup,
) -> Result<Vec<SocketAddr>, ResolveError> {
    let name = bare(host);
    let found = match name.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => lookup
            .find(name, port)
            .await
            .map_err(|source| ResolveError::Lookup {
                host: host.to_owned(),
                source,
            })?,
    };
    let kept: Vec<SocketAddr> = found
        .into_iter()
        .filter(|a| reach.permits(a.ip()))
        .collect();
    if kept.is_empty() {
        return Err(Refused {
            host: host.to_owned(),
        }
        .into());
    }
    Ok(kept)
}

pub fn check_literal(url: &url::Url, reach: &Reach) -> Result<(), Refused> {
    let ip = match url.host() {
        Some(url::Host::Ipv4(v4)) => IpAddr::V4(v4),
        Some(url::Host::Ipv6(v6)) => IpAddr::V6(v6),
        _ => return Ok(()),
    };
    if reach.permits(ip) {
        Ok(())
    } else {
        Err(Refused {
            host: ip.to_string(),
        })
    }
}

// reqwest asks the resolver for every new connection, so a name that later resolves
// somewhere else (DNS rebinding) is checked again; IP literals never reach a resolver
// and go through `check_literal` before every call instead.
#[derive(Debug)]
pub struct GuardedResolver {
    reach: Reach,
    lookup: Lookup,
}

impl GuardedResolver {
    pub fn new(reach: Reach, lookup: Lookup) -> GuardedResolver {
        GuardedResolver { reach, lookup }
    }
}

impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        let reach = self.reach.clone();
        let lookup = self.lookup.clone();
        Box::pin(async move {
            match resolve(&host, 0, &reach, &lookup).await {
                Ok(kept) => Ok(Box::new(kept.into_iter()) as reqwest::dns::Addrs),
                Err(ResolveError::Refused(r)) => Err(r.into()),
                Err(other) => Err(other.into()),
            }
        })
    }
}

pub fn refused_in(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(e);
    while let Some(c) = cause {
        if c.downcast_ref::<Refused>().is_some() {
            return true;
        }
        cause = c.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or_else(|_| unreachable!())
    }

    fn net(s: &str) -> IpNet {
        s.parse().unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn private_loopback_and_metadata_addresses_are_refused_unless_the_allowlist_names_them() {
        let default = Reach::public_only();
        for refused in [
            "127.0.0.1",
            "10.0.0.2",
            "192.168.1.10",
            "::1",
            "fd00::1",
            "169.254.169.254",
            "::ffff:10.0.0.1",
        ] {
            assert!(!default.permits(ip(refused)), "{refused}");
        }
        assert!(default.permits(ip("1.1.1.1")));
        let listed = Reach::networks(vec![net("127.0.0.0/8"), net("10.1.0.0/16")]);
        assert!(listed.permits(ip("127.0.0.1")));
        assert!(listed.permits(ip("10.1.2.3")));
        assert!(!listed.permits(ip("10.2.0.1")));
        assert!(!listed.permits(ip("::1")));
        assert!(listed.permits(ip("8.8.8.8")));
        let everything = Reach::networks(vec![net("0.0.0.0/0"), net("::/0")]);
        for metadata in [
            "169.254.169.254",
            "fd00:ec2::254",
            "168.63.129.16",
            "fe80::1",
        ] {
            assert!(!everything.permits(ip(metadata)), "{metadata}");
        }
        assert_eq!(Reach::networks(Vec::new()), Reach::public_only());
    }

    #[test]
    fn plain_http_reaches_only_listed_private_addresses() {
        let http = Reach::networks(vec![net("127.0.0.0/8")]).without_public();
        assert!(http.permits(ip("127.0.0.1")));
        assert!(!http.permits(ip("1.1.1.1")));
        assert!(
            !Reach::every_private_network()
                .without_public()
                .permits(ip("8.8.8.8"))
        );
        assert!(Reach::every_private_network().permits(ip("10.9.9.9")));
    }

    fn fixed(answers: Vec<Vec<&'static str>>) -> (Lookup, Arc<Mutex<usize>>) {
        let asked = Arc::new(Mutex::new(0usize));
        let count = asked.clone();
        let lookup = Lookup::new(move |_, port| {
            let mut n = count
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let answer = answers
                .get(*n)
                .or_else(|| answers.last())
                .cloned()
                .unwrap_or_default();
            *n += 1;
            let addrs: Vec<SocketAddr> = answer
                .iter()
                .map(|a| SocketAddr::new(ip(a), port))
                .collect();
            Box::pin(async move { Ok(addrs) })
        });
        (lookup, asked)
    }

    #[tokio::test]
    async fn a_name_that_resolves_to_a_private_address_is_refused() {
        let (lookup, _) = fixed(vec![vec!["10.0.0.5"]]);
        let err = resolve("vllm.example.com", 443, &Reach::public_only(), &lookup)
            .await
            .unwrap_err();
        assert!(matches!(err, ResolveError::Refused(_)), "{err}");
        assert_eq!(
            err.to_string(),
            "vllm.example.com resolves only to addresses this install does not allow"
        );
        let (mixed, _) = fixed(vec![vec!["169.254.169.254", "1.1.1.1"]]);
        let kept = resolve("mixed.example.com", 443, &Reach::public_only(), &mixed)
            .await
            .unwrap_or_default();
        assert_eq!(kept, [SocketAddr::new(ip("1.1.1.1"), 443)]);
        let literal = resolve("[::1]", 8000, &Reach::public_only(), &Lookup::default()).await;
        assert!(matches!(literal, Err(ResolveError::Refused(_))));
    }

    #[tokio::test]
    async fn every_new_connection_resolves_and_checks_the_name_again() {
        use reqwest::dns::Resolve as _;
        let (lookup, asked) = fixed(vec![vec!["1.1.1.1"], vec!["169.254.169.254"]]);
        let resolver = GuardedResolver::new(Reach::public_only(), lookup);
        let name = |s: &str| {
            s.parse::<reqwest::dns::Name>()
                .unwrap_or_else(|_| unreachable!())
        };
        let first: Vec<SocketAddr> = resolver
            .resolve(name("rebind.example.com"))
            .await
            .map(Iterator::collect)
            .unwrap_or_default();
        assert_eq!(first, [SocketAddr::new(ip("1.1.1.1"), 0)]);
        let second = resolver.resolve(name("rebind.example.com")).await;
        let err = second.err().unwrap_or_else(|| unreachable!());
        assert!(refused_in(err.as_ref()), "{err}");
        assert_eq!(
            *asked
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            2
        );
    }

    #[test]
    fn an_ip_literal_in_the_url_is_checked_without_a_resolver() {
        let url = |s: &str| url::Url::parse(s).unwrap_or_else(|_| unreachable!());
        let reach = Reach::public_only();
        assert!(check_literal(&url("http://169.254.169.254/latest"), &reach).is_err());
        assert!(check_literal(&url("https://[::1]:8443"), &reach).is_err());
        assert!(check_literal(&url("https://1.1.1.1"), &reach).is_ok());
        assert!(check_literal(&url("https://localhost:8000"), &reach).is_ok());
    }
}
