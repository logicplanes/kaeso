use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => match embedded_v4(v6) {
            Some(v4) => is_public_v4(v4),
            None => is_public_v6(v6),
        },
    }
}

pub fn is_link_local_or_metadata(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => match embedded_v4(v6) {
            Some(v4) => v4.is_link_local(),
            None => {
                let s = v6.segments();
                (s[0] & 0xffc0) == 0xfe80 || s == [0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254]
            }
        },
    }
}

fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    let low = Ipv4Addr::from(u32::from(s[6]) << 16 | u32::from(s[7]));
    match s {
        [0, 0, 0, 0, 0, 0xffff, _, _] => Some(low),
        [0, 0, 0, 0, 0, 0, _, _] if !ip.is_loopback() && !ip.is_unspecified() => Some(low),
        [0x64, 0xff9b, 0, 0, 0, 0, _, _] => Some(low),
        [0x64, 0xff9b, 1, ..] => Some(low),
        [0x2002, high, low16, ..] => Some(Ipv4Addr::from(u32::from(high) << 16 | u32::from(low16))),
        _ => None,
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..128).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && (b == 18 || b == 19))
        || a >= 240)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let first = segments.first().copied().unwrap_or(0);
    let second = segments.get(1).copied().unwrap_or(0);
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || (first & 0xffc0) == 0xfec0
        || (first == 0x2001 && (second == 0x0db8 || second == 0))
        || (first == 0x3fff && (second & 0xf000) == 0)
        || (first == 0x0100 && segments.get(1..4) == Some(&[0, 0, 0])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_local_and_cloud_metadata_addresses_are_known() {
        for ip in [
            "169.254.169.254",
            "169.254.0.1",
            "fe80::1",
            "fd00:ec2::254",
            "::ffff:169.254.169.254",
        ] {
            assert!(is_link_local_or_metadata(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "10.0.0.1",
            "127.0.0.1",
            "192.168.1.1",
            "fd00::1",
            "1.1.1.1",
            "::1",
        ] {
            assert!(!is_link_local_or_metadata(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn private_and_special_addresses_are_not_public() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "255.255.255.255",
            "192.0.2.1",
            "198.18.0.1",
            "::1",
            "::",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "64:ff9b::a9fe:a9fe",
            "64:ff9b:1::a00:1",
            "::7f00:1",
            "2002:a9fe:a9fe::1",
            "2001::1",
            "fec0::1",
            "100::1",
            "3fff::1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
            "2002:808:808::1",
        ] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
    }
}
