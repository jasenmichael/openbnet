//! Files the games download, and the BNFTP protocol that serves them.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::config;
use crate::wire::{Reader, Writer};

/// Games run the code inside these, so by default only files with a listed checksum are served.
const KNOWN: &[&str] = &[
    "ebb7d624774b8f68ce3f6d1e5cb784715a2909f27e3a35e78d13f787dcc41445", // IX86ExtraWork.mpq
    "c4cb275b310b1b268e7da64000fefbc8c3feaafb25e8830cd4c8955034f69027", // IX86ver1.mpq
    "687e14a318f78f1936f2ff1ee3c5ba5990b46460b3d2b4af6be11294fe2176e2", // PMACver1.mpq
    "b5deb6ffeb88de8f58942b67e75b0f28aaa6703947cc968e1a0a21d378d18fef", // ver-IX86-1.mpq
    "3bf355a8b56a8d36d85466ff6d6302bb2a03cc6665bc2a32990688d4c64ea37f", // XMACver1.mpq
];

/// Types that hold no code the games would run.
const PASSIVE: &[&str] = &["bni", "ini", "txt", "png", "smk", "mng", "pcx", "jpg", "save", "dat"];

const MAX_FILE: u64 = 16 * 1024 * 1024;

pub struct Files {
    directory: PathBuf,
    allow_unknown: bool,
}

pub struct File {
    pub data: Vec<u8>,
    /// Last change, as a Windows FILETIME.
    pub modified: u64,
}

impl Files {
    pub fn new(config: &config::Files) -> Files {
        Files { directory: config.directory.clone(), allow_unknown: config.allow_unknown }
    }

    /// The last change to a file, or 0 if it can't be served.
    pub async fn modified(&self, name: &str) -> u64 {
        self.read(name).await.map_or(0, |file| file.modified)
    }

    pub async fn read(&self, name: &str) -> Option<File> {
        if !is_plain_name(name) {
            return None;
        }
        let mut path = self.directory.join(name);
        let mut meta = tokio::fs::symlink_metadata(&path).await;
        if meta.is_err() {
            path = self.directory.join(fallback(name)?);
            meta = tokio::fs::symlink_metadata(&path).await;
        }
        let meta = meta.ok()?;
        if !meta.is_file() || meta.len() > MAX_FILE {
            return None;
        }
        let data = tokio::fs::read(&path).await.ok()?;
        if !self.may_serve(name, &data) {
            tracing::warn!(name, "refusing to serve a file with an unknown checksum");
            return None;
        }
        Some(File { data, modified: filetime(meta.modified().ok()?) })
    }

    fn may_serve(&self, name: &str, data: &[u8]) -> bool {
        let extension = name.rsplit_once('.').map_or("", |(_, ext)| ext).to_ascii_lowercase();
        if PASSIVE.contains(&extension.as_str()) || (self.allow_unknown && extension == "mpq") {
            return true;
        }
        let digest: String = Sha256::digest(data).iter().map(|byte| format!("{byte:02x}")).collect();
        KNOWN.contains(&digest.as_str())
    }
}

/// Games ask for texts by language, such as `tos_USA.txt`. One file can serve them all.
fn fallback(name: &str) -> Option<&'static str> {
    const TEXTS: &[(&str, &str)] = &[
        ("termsofservice-", "termsofservice.txt"),
        ("tos_", "termsofservice.txt"),
        ("tos-unicode_", "termsofservice.txt"),
        ("newaccount-", "newaccount.txt"),
        ("chathelp-war3-", "chathelp-war3.txt"),
    ];
    TEXTS.iter().find(|(prefix, _)| name.starts_with(prefix)).map(|(_, file)| *file)
}

fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b' '))
}

/// 100-nanosecond steps since 1601, as Windows counts time.
pub fn filetime(time: SystemTime) -> u64 {
    let since_unix = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    (since_unix.as_secs() + 11_644_473_600) * 10_000_000
}

const REQUEST_V1: u16 = 0x0100;

/// Serves one BNFTP connection, after its protocol byte.
pub async fn serve<S: AsyncRead + AsyncWrite + Unpin>(files: &Files, stream: &mut S) -> anyhow::Result<()> {
    let size = usize::from(stream.read_u16_le().await?);
    anyhow::ensure!((34..=512).contains(&size), "bad request size {size}");
    let mut body = vec![0; size - 2];
    stream.read_exact(&mut body).await?;
    let mut r = Reader::new(&body);
    let version = r.u16()?;
    anyhow::ensure!(version == REQUEST_V1, "unsupported BNFTP version {version:#06x}");
    r.skip(8)?; // platform and product
    let (ad, extension, start) = (r.u32()?, r.u32()?, r.u32()? as usize);
    r.skip(8)?; // the client's copy's time
    let name = r.ascii(128)?;

    // Games save whatever they are sent, so a missing file must not look like an empty one.
    let Some(file) = files.read(name).await else {
        tracing::warn!(name, "a game asked for a file that can't be served");
        return Ok(());
    };
    let header_size = 24 + name.len() + 1;
    let header = Writer::new()
        .u16(header_size as u16)
        .u16(0)
        .u32(file.data.len() as u32)
        .u32(ad)
        .u32(extension)
        .u64(file.modified)
        .cstr(name)
        .finish();
    stream.write_all(&header).await?;
    if let Some(rest) = file.data.get(start..) {
        stream.write_all(rest).await?;
    }
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(dir: &std::path::Path) -> Files {
        Files::new(&config::Files { directory: dir.into(), allow_unknown: false })
    }

    #[tokio::test]
    async fn serves_passive_files_but_not_unknown_code() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["icons.bni", "evil.mpq", "evil.dll", ".hidden.txt"] {
            std::fs::write(dir.path().join(name), b"data").unwrap();
        }
        let files = files(dir.path());
        assert_eq!(files.read("icons.bni").await.unwrap().data, b"data");
        for name in ["evil.mpq", "evil.dll", ".hidden.txt", "../x.txt", "/etc/passwd", "", "missing.txt"] {
            assert!(files.read(name).await.is_none(), "{name}");
        }
    }

    #[tokio::test]
    async fn one_text_serves_every_language() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("termsofservice.txt"), b"terms").unwrap();
        let files = files(dir.path());
        assert_eq!(files.read("tos_USA.txt").await.unwrap().data, b"terms");
        std::fs::write(dir.path().join("tos_DEU.txt"), b"Bedingungen").unwrap();
        assert_eq!(files.read("tos_DEU.txt").await.unwrap().data, b"Bedingungen");
        assert!(files.read("newaccount-enUS.txt").await.is_none());
    }

    #[test]
    fn filetime_counts_from_1601() {
        assert_eq!(filetime(UNIX_EPOCH), 116_444_736_000_000_000);
    }
}
