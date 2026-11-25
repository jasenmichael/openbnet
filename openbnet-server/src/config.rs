//! The server's settings, read from one TOML file. Every setting has a default.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::range::Range;
use crate::wire::Tag;

/// The settings' name in the data folder, and what is written there to start with.
pub const FILE: &str = "openbnet.toml";
const TEMPLATE: &str = include_str!("../assets/openbnet.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    pub database: Database,
    #[serde(default)]
    pub files: Files,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub accounts: Accounts,
    #[serde(default)]
    pub chat: Chat,
    #[serde(default)]
    pub games: Games,
    #[serde(default)]
    pub bots: Bots,
    #[serde(default)]
    pub irc: Irc,
    #[serde(default)]
    pub http: Http,
    #[serde(rename = "build", default = "default_builds")]
    pub builds: Vec<Build>,
    #[serde(rename = "ad", default)]
    pub ads: Vec<Ad>,
    #[serde(rename = "channel", default = "default_channels")]
    pub channels: Vec<Channel>,
}

/// The builds listed in the template.
fn default_builds() -> Vec<Build> {
    #[derive(Deserialize)]
    struct Template {
        build: Vec<Build>,
    }
    let template: Template = toml::from_str(TEMPLATE).expect("the template is valid");
    template.build
}

/// The channels listed in the template.
fn default_channels() -> Vec<Channel> {
    #[derive(Deserialize)]
    struct Example {
        channel: Vec<Channel>,
    }
    let example: Example = toml::from_str(TEMPLATE).expect("the template is valid");
    example.channel
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Server {
    pub name: String,
    /// Where games connect, over TCP and UDP.
    pub listen: SocketAddr,
    pub max_connections: usize,
    pub max_connections_per_ip: usize,
    /// How long a connection may go without logging in.
    pub login_timeout_secs: u64,
    pub ping_interval_secs: u64,
    /// The warning players get before the server stops.
    pub shutdown_secs: u64,
    /// Products allowed to connect. Empty means all supported ones.
    pub products: Vec<String>,
    /// Refuse game versions that are unknown or fail the version check.
    pub strict_versions: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    /// Empty means `openbnet.db` in the data folder. Otherwise `postgres://...` or `sqlite://...`.
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Files {
    pub directory: PathBuf,
    /// Serve version-check files whose checksum isn't known. Game clients run the code inside them.
    pub allow_unknown: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Network {
    /// The address players outside `local` should use for hosts inside it.
    pub public_ip: Option<Ipv4Addr>,
    pub local: Vec<Range>,
    #[serde(rename = "override")]
    pub overrides: Vec<Override>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub host: Ipv4Addr,
    pub public: SocketAddrV4,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Accounts {
    pub allow_new: bool,
    /// Symbols allowed in names besides letters and digits.
    pub symbols: String,
    /// Wrong passwords allowed per address in `window_secs`. 0 means no limit.
    pub max_failures: usize,
    pub window_secs: u64,
    /// New accounts allowed per address in `signup_window_secs`. 0 means no limit.
    pub max_signups: usize,
    pub signup_window_secs: u64,
}

/// Text chat on the game port. Passwords travel unencrypted.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bots {
    pub enabled: bool,
    /// Accounts that may use it. `*` means all.
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Irc {
    /// Unencrypted.
    pub listen: Option<SocketAddr>,
    pub tls_listen: Option<SocketAddr>,
    /// PEM files.
    pub certificate: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

/// The HTTP and WebSocket API. Put a reverse proxy with TLS in front of it.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Http {
    pub listen: Option<SocketAddr>,
    pub max_connections: usize,
    /// Believe X-Forwarded-For. Only for use behind a proxy that sets it.
    pub behind_proxy: bool,
    /// The password of the administration API, which is off when this is empty.
    pub admin_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chat {
    pub welcome: Vec<String>,
    pub flood_lines: usize,
    pub flood_secs: u64,
    pub flood_disconnect_lines: usize,
    pub max_line: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Games {
    pub hide_private: bool,
    pub hide_started: bool,
    pub disconnect_is_loss: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub name: String,
    /// The name clients ask for. When set, numbered copies open as the channel fills.
    #[serde(default)]
    pub alias: Option<String>,
    /// The games it is offered to. Empty means all.
    #[serde(default)]
    pub products: Vec<String>,
    #[serde(default)]
    pub max: Option<usize>,
    #[serde(default)]
    pub moderated: bool,
    #[serde(default)]
    pub staff_only: bool,
    #[serde(default)]
    pub topic: String,
}

/// A game build to accept besides those in PvPGN's list.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    #[serde(deserialize_with = "parse_tag")]
    pub product: Tag,
    #[serde(deserialize_with = "parse_version")]
    pub version: u32,
    /// As games report it; see the log.
    pub checksum: u32,
    /// Builds with the same tag see each other's games.
    #[serde(default)]
    pub tag: Option<String>,
}

fn parse_tag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Tag, D::Error> {
    let text = String::deserialize(deserializer)?;
    Tag::parse(&text).ok_or_else(|| D::Error::custom(format!("{text} isn't a product, such as W2BN")))
}

fn parse_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let text = String::deserialize(deserializer)?;
    crate::versions::parse_version(&text)
        .map_err(|_| D::Error::custom(format!("{text} isn't a version, such as 2.0.2.0")))
}

/// A banner shown in the games. The file must be in the files directory.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ad {
    pub file: String,
    pub url: String,
    #[serde(default)]
    pub product: Option<String>,
}

impl Network {
    /// The address `viewer` should use to reach a game host.
    pub fn host_address(&self, viewer: IpAddr, host: SocketAddrV4) -> SocketAddrV4 {
        let local = |ip| self.local.iter().any(|range| range.contains(ip));
        if local(viewer) {
            return host;
        }
        if let Some(found) = self.overrides.iter().find(|o| o.host == *host.ip()) {
            return found.public;
        }
        match self.public_ip {
            Some(ip) if local(IpAddr::V4(*host.ip())) => SocketAddrV4::new(ip, host.port()),
            _ => host,
        }
    }
}

impl Config {
    /// Reads the settings in a data folder, first creating the folder and whatever it lacks.
    pub fn open(folder: &Path) -> anyhow::Result<Config> {
        let path = folder.join(FILE);
        if !path.exists() {
            let create = || -> std::io::Result<()> {
                std::fs::create_dir_all(folder)?;
                std::fs::write(&path, TEMPLATE)
            };
            create().with_context(|| format!("creating {}", path.display()))?;
        }
        let config = Config::load(folder)?;
        // Only worth a warning: accounts and chat work without it.
        if let Err(error) = std::fs::create_dir_all(&config.files.directory) {
            tracing::warn!(%error, directory = %config.files.directory.display(), "could not create");
        }
        Ok(config)
    }

    pub fn load(folder: &Path) -> anyhow::Result<Config> {
        let path = folder.join(FILE);
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut config: Config = toml::from_str(&text).with_context(|| format!("in {}", path.display()))?;
        config.files.directory = folder.join(&config.files.directory);
        for path in [&mut config.irc.certificate, &mut config.irc.key].into_iter().flatten() {
            *path = folder.join(&*path);
        }
        if config.database.url.is_empty() {
            config.database.url = sqlite_url(&folder.join("openbnet.db"))?;
        }
        Ok(config)
    }
}

fn sqlite_url(path: &Path) -> anyhow::Result<String> {
    let path = path.to_str().context("the data folder's name isn't UTF-8")?;
    let mut url = String::from("sqlite://");
    for byte in path.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_' => url.push(byte.into()),
            _ => url += &format!("%{byte:02x}"),
        }
    }
    Ok(url + "?mode=rwc")
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str("").expect("every setting has a default")
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            name: "openbnet".into(),
            listen: SocketAddr::from(([0, 0, 0, 0], 6112)),
            max_connections: 2000,
            max_connections_per_ip: 16,
            login_timeout_secs: 120,
            ping_interval_secs: 120,
            shutdown_secs: 5,
            products: vec!["W2BN".into()],
            strict_versions: false,
        }
    }
}

impl Default for Files {
    fn default() -> Self {
        Files { directory: "files".into(), allow_unknown: false }
    }
}

impl Default for Accounts {
    fn default() -> Self {
        Accounts {
            allow_new: true,
            symbols: "-_[]".into(),
            max_failures: 10,
            window_secs: 300,
            max_signups: 5,
            signup_window_secs: 3600,
        }
    }
}

impl Default for Chat {
    fn default() -> Self {
        Chat { welcome: Vec::new(), flood_lines: 5, flood_secs: 5, flood_disconnect_lines: 10, max_line: 200 }
    }
}

impl Default for Http {
    fn default() -> Self {
        Http {
            listen: Some(SocketAddr::from(([127, 0, 0, 1], 6180))),
            max_connections: 256,
            behind_proxy: false,
            admin_token: String::new(),
        }
    }
}

impl Default for Games {
    fn default() -> Self {
        Games { hide_private: true, hide_started: true, disconnect_is_loss: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_is_valid_and_matches_the_defaults() {
        let example: Config = toml::from_str(TEMPLATE).unwrap();
        assert_eq!(format!("{example:?}"), format!("{:?}", Config::default()));
    }

    #[tokio::test]
    async fn a_new_data_folder_is_ready_to_use() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("my data? 100%");
        let config = Config::open(&folder).unwrap();
        assert!(folder.join(FILE).is_file() && folder.join("files").is_dir());
        assert_eq!(config.files.directory, folder.join("files"));
        crate::store::Store::open(&config.database.url).await.unwrap();
        assert!(folder.join("openbnet.db").is_file());
        std::fs::write(folder.join(FILE), "[irc]\ncertificate = 'c.pem'\nkey = '/etc/k.pem'").unwrap();
        let config = Config::open(&folder).unwrap();
        assert_eq!(config.irc.certificate, Some(folder.join("c.pem")), "kept, and relative to the folder");
        assert_eq!(config.irc.key, Some("/etc/k.pem".into()));
    }

    #[test]
    fn every_setting_is_documented() {
        let docs = include_str!("../docs/CONFIGURATION.md");
        for line in TEMPLATE.lines() {
            let line = line.strip_prefix("# ").unwrap_or(line);
            if let Some((name, _)) = line.split_once(" = ") {
                assert!(docs.contains(&format!("`{name}`")), "{name}");
            }
        }
    }

    #[test]
    fn unknown_settings_are_refused() {
        assert!(toml::from_str::<Config>("[server]\nnmae = 'x'").is_err());
    }

    #[test]
    fn translates_hosts_only_for_outside_players() {
        let net: Network = toml::from_str(
            "public_ip = '203.0.113.9'\nlocal = ['192.168.1.0/24']\n\
             [[override]]\nhost = '192.168.1.50'\npublic = '203.0.113.9:6118'",
        )
        .unwrap();
        let inside = Ipv4Addr::new(192, 168, 1, 7);
        let outside = Ipv4Addr::new(198, 51, 100, 4);
        let host = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), 6112);
        let special = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 50), 6112);
        assert_eq!(net.host_address(inside.into(), host), host);
        assert_eq!(net.host_address(outside.into(), host), "203.0.113.9:6112".parse().unwrap());
        assert_eq!(net.host_address(outside.into(), special), "203.0.113.9:6118".parse().unwrap());
        let remote = SocketAddrV4::new(outside, 6112);
        assert_eq!(net.host_address(inside.into(), remote), remote);
    }
}
