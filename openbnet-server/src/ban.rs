//! Banning addresses, from chat or the API.

use std::net::IpAddr;

use anyhow::Result;

use crate::hub::SessionId;
use crate::range::Range;
use crate::server::Server;
use crate::store::{IpBan, Role, unix_now};

/// Bans an address, a range, or the address a player last used. `hours` 0 means for good. The inner error says why
/// it can't be done.
pub async fn add(
    server: &Server,
    asker: Option<SessionId>,
    target: &str,
    hours: u32,
    reason: &str,
) -> Result<Result<Range, &'static str>> {
    let range = match target.parse::<Range>() {
        Ok(range) => range,
        Err(_) => match address_of(server, target).await? {
            Some(ip) => Range::of(ip),
            None => return Ok(Err("That is no address, and no player whose address is known.")),
        },
    };
    if range.includes_this_computer() {
        return Ok(Err("That includes the server's own computer."));
    }
    if range.is_wide() {
        return Ok(Err("That is too many addresses: at most a /16, or a /32 with IPv6."));
    }
    let by = {
        let hub = server.hub.lock();
        match asker.map(|id| hub.session(id)) {
            None => "the API".to_owned(),
            Some(None) => return Ok(Err("You are no longer connected.")),
            Some(Some(asker)) => {
                if range.contains(asker.ip) {
                    return Ok(Err("That includes your own address."));
                }
                let spares = |role: Role| role >= Role::Moderator && role >= asker.role();
                if hub.sessions().any(|session| range.contains(session.ip) && spares(session.role())) {
                    return Ok(Err("That includes a moderator or administrator."));
                }
                asker.name().to_owned()
            }
        }
    };
    let now = unix_now();
    let until = if hours == 0 { 0 } else { now + i64::from(hours) * 3600 };
    let ban = IpBan { range, reason: reason.to_owned(), by, until };
    server.store.add_ip_ban(&ban).await?;
    tracing::info!(%range, by = ban.by, hours, reason, "addresses banned");
    server.hub.lock().add_ban(ban);
    Ok(Ok(range))
}

/// False if there was no such ban.
pub async fn remove(server: &Server, range: Range) -> Result<bool> {
    let removed = server.store.remove_ip_ban(range).await?;
    server.hub.lock().remove_ban(range);
    tracing::info!(%range, removed, "ban lifted");
    Ok(removed)
}

async fn address_of(server: &Server, name: &str) -> Result<Option<IpAddr>> {
    if let Some(ip) = server.hub.lock().find_user(name).map(|session| session.ip) {
        return Ok(Some(ip));
    }
    Ok(server.store.last_address(name).await?.and_then(|ip| ip.parse().ok()))
}
