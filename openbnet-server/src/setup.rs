//! Writing the settings for the usual ways to run a server.

use std::io::{BufRead, Write};
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs, UdpSocket};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::config::{Config, FILE};
use crate::range::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Players {
    /// Only on this computer.
    Local,
    /// On this network.
    Lan,
    /// Friends on a VPN such as Tailscale.
    Vpn,
    /// Anyone, over the internet.
    Internet,
}

pub struct Answers {
    pub players: Players,
    /// What players enter as the gateway: an address or a domain.
    pub address: String,
    pub allow_new: bool,
}

/// This computer's address towards another one. Nothing is sent.
fn address_towards(other: &str) -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect((other, 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

pub fn network_address() -> Option<Ipv4Addr> {
    address_towards("192.0.2.1")
}

/// Carriers and VPNs such as Tailscale use 100.64.0.0/10.
fn is_shared(ip: Ipv4Addr) -> bool {
    Range::new(Ipv4Addr::new(100, 64, 0, 0).into(), 10).contains(ip.into())
}

fn is_inside(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || is_shared(ip)
}

/// An address on a VPN, if this computer seems to be on Tailscale.
fn vpn_address() -> Option<Ipv4Addr> {
    address_towards("100.100.100.100").filter(|ip| is_shared(*ip))
}

pub fn suggested_address(players: Players) -> Option<String> {
    match players {
        Players::Local => Some(Ipv4Addr::LOCALHOST.to_string()),
        Players::Lan => network_address().map(|ip| ip.to_string()),
        Players::Vpn => vpn_address().map(|ip| ip.to_string()),
        Players::Internet => network_address().filter(|ip| !is_inside(*ip)).map(|ip| ip.to_string()),
    }
}

/// Asks for whatever wasn't given.
pub fn ask(players: Option<Players>, address: Option<String>, allow_new: Option<bool>) -> Result<Answers> {
    let players = match players {
        Some(players) => players,
        None => {
            println!("Who will play?");
            println!("  1. only this computer");
            println!("  2. this network");
            println!("  3. friends on a VPN, such as Tailscale");
            println!("  4. anyone, over the internet");
            match prompt("Choose 1 to 4", Some("2"))?.as_str() {
                "1" => Players::Local,
                "2" => Players::Lan,
                "3" => Players::Vpn,
                "4" => Players::Internet,
                other => bail!("{other} isn't one of the choices"),
            }
        }
    };
    let address = match (address, players) {
        (Some(address), _) => address,
        (None, Players::Local) => Ipv4Addr::LOCALHOST.to_string(),
        (None, _) => {
            let question = match players {
                Players::Internet => "Your domain or public address",
                _ => "This computer's address there",
            };
            prompt(question, suggested_address(players).as_deref())?
        }
    };
    let allow_new = match allow_new {
        Some(allow_new) => allow_new,
        None => match prompt("May players make their own accounts? (yes/no)", Some("yes"))?.as_str() {
            "yes" | "y" => true,
            "no" | "n" => false,
            other => bail!("{other} is neither yes nor no"),
        },
    };
    Ok(Answers { players, address, allow_new })
}

fn prompt(question: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(default) => print!("{question} [{default}]: "),
        None => print!("{question}: "),
    }
    std::io::stdout().flush()?;
    let mut line = String::new();
    ensure!(std::io::stdin().lock().read_line(&mut line)? > 0, "no answer");
    match (line.trim(), default) {
        ("", Some(default)) => Ok(default.to_owned()),
        ("", None) => bail!("no answer"),
        (answer, _) => Ok(answer.to_owned()),
    }
}

fn valid_host(host: &str) -> bool {
    !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
}

/// Writes the settings, keeping everything else in them, and says what to do next.
pub fn apply(folder: &Path, answers: &Answers, own: Option<Ipv4Addr>) -> Result<String> {
    let Answers { players, address, allow_new } = answers;
    ensure!(valid_host(address), "{address} isn't an address or a domain");
    Config::open(folder)?;
    let path = folder.join(FILE);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut settings: DocumentMut = text.parse().with_context(|| format!("in {}", path.display()))?;

    let listen = if *players == Players::Local { "127.0.0.1:6112" } else { "0.0.0.0:6112" };
    table(&mut settings, "server")?["listen"] = value(listen);
    table(&mut settings, "accounts")?["allow_new"] = value(*allow_new);
    let network = table(&mut settings, "network")?;
    network.remove("public_ip");
    network["local"] = value(Array::new());
    let behind_router = own.filter(|own| is_inside(*own));
    let mut inside = None;
    if let (Players::Internet, Some(own)) = (players, behind_router) {
        let public = public_address(address)?;
        let range = Range::new(own.into(), 24);
        network["public_ip"] = value(public.to_string());
        network["local"] = value(Array::from_iter([range.to_string()]));
        inside = Some((own, range));
    }

    // Never leave a broken file behind.
    let text = settings.to_string();
    toml::from_str::<Config>(&text).context("the new settings would be invalid")?;
    let new = folder.join("openbnet.toml.new");
    std::fs::write(&new, text).with_context(|| format!("writing {}", new.display()))?;
    std::fs::rename(&new, &path).with_context(|| format!("replacing {}", path.display()))?;

    let mut next = format!("Wrote {}.\n\nPlayers' gateway: {address}\n", path.display());
    let mut add = |line: String| next = format!("{next}{line}\n");
    add(format!("With wc2bne: wc2bne gateway add {address} \"My server\""));
    match (players, inside) {
        (Players::Local, _) => add("Nobody else can connect.".into()),
        (Players::Lan, _) => add("Allow TCP and UDP 6112 through this computer's firewall.".into()),
        (Players::Vpn, _) => add("Everyone, you included, must use the VPN. No ports need forwarding.".into()),
        (Players::Internet, None) => {
            add("Allow TCP and UDP 6112 through the firewall, here and at your provider.".into())
        }
        (Players::Internet, Some((own, range))) => {
            add(format!("On your router, forward TCP and UDP 6112 to {own}."));
            add(format!("Players in {range} use {own} as their gateway instead."));
            add("To host games for others from inside, forward that computer's UDP game port to it too.".into());
        }
    }
    if *players != Players::Local {
        let here = inside.map_or(address.clone(), |(own, _)| own.to_string());
        add(format!("To play on this computer, use {here} too, not 127.0.0.1."));
    }
    if !allow_new {
        add("Make each player's account with: openbnet create-account NAME".into());
    }
    add("Make yourself an administrator with: openbnet create-account NAME --role admin".into());
    add("A changed listening address needs a restart: openbnet serve".into());
    Ok(next)
}

fn table<'a>(settings: &'a mut DocumentMut, name: &str) -> Result<&'a mut Table> {
    let item = settings.entry(name).or_insert(Item::Table(Table::new()));
    item.as_table_mut().with_context(|| format!("[{name}] isn't a table"))
}

/// The address outside players will reach. Settings need a number, so a domain is looked up.
fn public_address(address: &str) -> Result<Ipv4Addr> {
    let found = match address.parse::<Ipv4Addr>() {
        Ok(ip) => ip,
        Err(_) => (address, 6112)
            .to_socket_addrs()
            .with_context(|| format!("looking up {address}; give your public address instead, with --address"))?
            .find_map(|found| match found.ip() {
                IpAddr::V4(ip) => Some(ip),
                IpAddr::V6(_) => None,
            })
            .with_context(|| format!("{address} has no IPv4 address"))?,
    };
    ensure!(!is_inside(found), "{found} isn't a public address; give yours with --address");
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(players: Players, address: &str, own: &str) -> (Config, String, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let answers = Answers { players, address: address.into(), allow_new: false };
        let next = apply(dir.path(), &answers, Some(own.parse().unwrap())).unwrap();
        (Config::load(dir.path()).unwrap(), next, dir)
    }

    #[test]
    fn each_kind_of_server_gets_its_settings() {
        let (config, _, _dir) = run(Players::Local, "127.0.0.1", "192.168.1.10");
        assert!(config.server.listen.ip().is_loopback());
        let (config, next, _dir) = run(Players::Lan, "192.168.1.10", "192.168.1.10");
        assert!(config.server.listen.ip().is_unspecified() && config.network.public_ip.is_none());
        assert!(next.contains("gateway add 192.168.1.10") && !config.accounts.allow_new);
        let (config, next, _dir) = run(Players::Internet, "203.0.113.9", "192.168.1.10");
        assert_eq!(config.network.public_ip, Some("203.0.113.9".parse().unwrap()));
        assert_eq!(config.network.local, ["192.168.1.0/24".parse().unwrap()]);
        assert!(next.contains("forward TCP and UDP 6112 to 192.168.1.10"));
        assert!(next.contains("use 192.168.1.10 too"));
        let (config, _, _dir) = run(Players::Internet, "bnet.example.com", "203.0.113.9");
        assert!(config.network.public_ip.is_none() && config.network.local.is_empty(), "a VPS needs neither");
    }

    #[test]
    fn other_settings_and_comments_are_kept() {
        let (_, _, dir) = run(Players::Internet, "203.0.113.9", "192.168.1.10");
        let path = dir.path().join(FILE);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("name = \"openbnet\"", "name = \"Mine\" # my comment")).unwrap();
        let answers = Answers { players: Players::Lan, address: "192.168.1.10".into(), allow_new: true };
        apply(dir.path(), &answers, None).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my comment") && text.contains("[[channel]]"));
        let config = Config::load(dir.path()).unwrap();
        assert!(config.server.name == "Mine" && config.network.public_ip.is_none() && config.accounts.allow_new);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2, "settings and files/ only");
    }

    #[test]
    fn nonsense_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let own = Some("192.168.1.10".parse().unwrap());
        for (players, address) in [
            (Players::Lan, ""),
            (Players::Lan, "a b"),
            (Players::Lan, "x\"y"),
            (Players::Internet, "192.168.1.10"),
        ] {
            let answers = Answers { players, address: address.into(), allow_new: true };
            assert!(apply(dir.path(), &answers, own).is_err(), "{address}");
        }
    }
}
