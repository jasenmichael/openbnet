//! Which version-check file and equation each game build is given, and which builds are known.
//!
//! The data is PvPGN's `versioncheck.json`.

use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::wire::Tag;

const WINDOWS: Tag = Tag::new(b"IX86");

pub struct Versions {
    checks: HashMap<(Tag, Tag, u32), Check>,
}

pub struct Check {
    pub file: String,
    pub equation: String,
    builds: Vec<Build>,
}

struct Build {
    version: u32,
    checksum: u32,
    tag: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict<'a> {
    /// A known build, with a tag shared by builds that can play together.
    Known(&'a str),
    /// A known version whose files don't match the expected checksum.
    Modified(&'a str),
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCheck {
    check_revision_file: String,
    equation: String,
    entries: Vec<RawBuild>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBuild {
    version: String,
    hash: String,
    version_tag: String,
}

type Raw = HashMap<String, HashMap<String, HashMap<String, RawCheck>>>;

impl Versions {
    pub fn load() -> Result<Versions> {
        let raw: Raw = serde_json::from_str(include_str!("../assets/versions.json"))?;
        let mut checks = HashMap::new();
        for (product, platforms) in raw {
            let product = Tag::parse(&product).context("bad product")?;
            for (platform, versions) in platforms {
                let platform = Tag::parse(&platform).context("bad platform")?;
                for (version, raw) in versions {
                    let builds = raw.entries.into_iter().map(Build::parse).collect::<Result<_>>()?;
                    let check = Check { file: raw.check_revision_file, equation: raw.equation, builds };
                    checks.insert((product, platform, parse_hex(&version)?), check);
                }
            }
        }
        Ok(Versions { checks })
    }

    pub fn check(&self, product: Tag, platform: Tag, version_byte: u32) -> (&str, &str) {
        match self.checks.get(&(product, platform, version_byte)) {
            Some(check) => (&check.file, &check.equation),
            None => ("ver-IX86-1.mpq", "A=42 B=42 C=42 4 A=A^S B=B^B C=C^C A=A^S"),
        }
    }

    /// The version-check files a game's known Windows builds need.
    pub fn files(&self, product: Tag) -> BTreeSet<&str> {
        let checks =
            self.checks.iter().filter(|((checked, platform, _), _)| (*checked, *platform) == (product, WINDOWS));
        checks.map(|(_, check)| check.file.as_str()).collect()
    }

    pub fn verdict(&self, key: (Tag, Tag, u32), version: u32, checksum: u32) -> Verdict<'_> {
        let builds = self.checks.get(&key).map_or(&[][..], |check| &check.builds);
        let mut verdict = Verdict::Unknown;
        for build in builds.iter().filter(|build| build.version == version) {
            if build.checksum == checksum {
                return Verdict::Known(&build.tag);
            }
            verdict = Verdict::Modified(&build.tag);
        }
        verdict
    }
}

impl Build {
    fn parse(raw: RawBuild) -> Result<Build> {
        let mut version = 0u32;
        let mut parts = 0;
        for part in raw.version.split('.') {
            version = (version << 8) | u32::from(part.parse::<u8>().with_context(|| raw.version.clone())?);
            parts += 1;
        }
        version = version.checked_shl(8 * (4u32.saturating_sub(parts))).unwrap_or(0);
        Ok(Build { version, checksum: parse_hex(&raw.hash)?, tag: raw.version_tag })
    }
}

fn parse_hex(text: &str) -> Result<u32> {
    let digits = text.strip_prefix("0x").unwrap_or(text);
    u32::from_str_radix(digits, 16).with_context(|| format!("bad number {text}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::product::WARCRAFT2;

    #[test]
    fn recognises_both_warcraft_2_builds() {
        let versions = Versions::load().unwrap();
        let key = (WARCRAFT2, Tag::new(b"IX86"), 0x4f);
        assert_eq!(versions.verdict(key, 0x0200_0200, 0xb52b_ad87), Verdict::Known("W2BN_202A"));
        assert_eq!(versions.verdict(key, 0x0200_0201, 0xff0d_4c4a), Verdict::Known("W2BN_202B"));
        assert_eq!(versions.verdict(key, 0x0200_0201, 1), Verdict::Modified("W2BN_202B"));
        assert_eq!(versions.verdict(key, 0x0900_0000, 1), Verdict::Unknown);
        assert!(versions.files(WARCRAFT2).contains("ver-IX86-1.mpq"));
        assert!(versions.check(WARCRAFT2, Tag::new(b"IX86"), 0x4f).0.ends_with(".mpq"));
    }
}
