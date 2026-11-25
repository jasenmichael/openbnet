//! Ranges of addresses, such as `192.168.1.0/24`.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct Range {
    base: IpAddr,
    bits: u8,
}

impl Range {
    pub fn new(ip: IpAddr, bits: u8) -> Range {
        Range { base: masked(ip, bits), bits }
    }

    /// Whoever has an address: that one, or with IPv6 the /64 that usually comes with it.
    pub fn of(ip: IpAddr) -> Range {
        Range::new(ip, if ip.is_ipv4() { 32 } else { 64 })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        self.base.is_ipv4() == ip.is_ipv4() && masked(ip, self.bits) == self.base
    }

    /// Too wide to be banned by mistake: more than a /16, or a /32 with IPv6.
    pub fn is_wide(&self) -> bool {
        self.bits < if self.base.is_ipv4() { 16 } else { 32 }
    }

    pub fn includes_this_computer(&self) -> bool {
        ["127.0.0.1", "::1"].iter().any(|ip| ip.parse().is_ok_and(|ip| self.contains(ip)))
            || self.base.is_loopback()
    }
}

fn masked(ip: IpAddr, bits: u8) -> IpAddr {
    match ip {
        IpAddr::V4(ip) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(bits.min(32))).unwrap_or(0);
            IpAddr::V4((u32::from(ip) & mask).into())
        }
        IpAddr::V6(ip) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(bits.min(128))).unwrap_or(0);
            IpAddr::V6((u128::from(ip) & mask).into())
        }
    }
}

impl FromStr for Range {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Range> {
        let (ip, bits) = match text.split_once('/') {
            Some((ip, bits)) => (ip, Some(bits)),
            None => (text, None),
        };
        let ip: IpAddr = ip.parse().with_context(|| format!("{text} isn't an address"))?;
        let Some(bits) = bits else { return Ok(Range::of(ip)) };
        let bits: u8 = bits.parse().with_context(|| format!("{text} isn't a range of addresses"))?;
        ensure!(bits <= if ip.is_ipv4() { 32 } else { 128 }, "{text} isn't a range of addresses");
        Ok(Range::new(ip, bits))
    }
}

impl TryFrom<String> for Range {
    type Error = anyhow::Error;

    fn try_from(text: String) -> Result<Range> {
        text.parse()
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.base, self.bits) {
            (IpAddr::V4(ip), 32) => write!(f, "{ip}"),
            (ip, bits) => write!(f, "{ip}/{bits}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_contain_what_they_say() {
        let range = |text: &str| text.parse::<Range>().unwrap();
        let ip = |text: &str| text.parse::<IpAddr>().unwrap();
        assert!(range("192.168.1.77/24").contains(ip("192.168.1.1")));
        assert!(!range("192.168.1.0/24").contains(ip("192.168.2.1")));
        assert!(range("0.0.0.0/0").contains(ip("8.8.8.8")) && !range("0.0.0.0/0").contains(ip("::1")));
        assert!(range("2001:db8::1").contains(ip("2001:db8::ffff")), "a whole /64");
        assert_eq!(range("203.0.113.9").to_string(), "203.0.113.9");
        assert_eq!(range("192.168.1.77/24").to_string(), "192.168.1.0/24");
        assert_eq!(range("2001:db8::1").to_string(), "2001:db8::/64");
        for bad in ["", "x", "1.2.3", "1.2.3.4/33", "1.2.3.4/-1", "::/129", "1.2.3.4/"] {
            assert!(bad.parse::<Range>().is_err(), "{bad}");
        }
        assert!(range("10.0.0.0/8").is_wide() && !range("10.0.0.0/16").is_wide());
        assert!(range("127.0.0.0/8").includes_this_computer() && range("0.0.0.0/0").includes_this_computer());
        assert!(range("::1").includes_this_computer() && !range("10.0.0.0/8").includes_this_computer());
    }
}
