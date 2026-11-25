//! Commands typed in chat, starting with a slash.

use anyhow::Result;

use crate::ban;
use crate::bncs;
use crate::hub::{Event, EventKind, Hub, Session, SessionId};
use crate::server::Server;
use crate::store::{Account, Role, unix_now};
use crate::wire::product;

const HELP: &[&str] = &[
    "/w <user> <message>: send a private message. /r <message>: reply to the last one.",
    "/me <text>: act. /away [why], /dnd [why]: mark yourself away or busy; repeat to clear.",
    "/whois <user>, /whoami, /where <user>, /who <channel>: find people.",
    "/join <channel>, /rejoin, /channels, /games: move around.",
    "/ignore <user>, /unignore <user>: hide someone's messages.",
    "/f add <user>, /f remove <user>, /f list, /f msg <text>: friends. /topic: the channel's topic.",
    "/stats [user], /time, /users, /uptime, /latency, /version, /motd",
];
const OPERATOR_HELP: &str = "/kick <user> [why], /ban <user> [why], /unban <user>, /op <user>, /deop <user>";
const MODERATOR_HELP: &str = "/announce <text>, /kill <user>, /mute <user>, /unmute <user>, /lock <user>, /unlock <user>, \
    /ipban <user or address> [hours] [why], /ipunban <address>, /ipbans";

/// Handles something typed: a command, or talk.
pub async fn say(server: &Server, id: SessionId, text: &[u8]) -> Result<()> {
    if text.first() == Some(&b'/') {
        return run(server, id, text).await;
    }
    let mut hub = server.hub.lock();
    if hub.channel_name(id).is_some() && hub.allow_line(id, text) {
        hub.talk(id, EventKind::Talk, text);
    }
    Ok(())
}

pub async fn run(server: &Server, id: SessionId, text: &[u8]) -> Result<()> {
    let (command, rest) = split(text);
    let command = String::from_utf8_lossy(command).to_ascii_lowercase();
    if !server.hub.lock().allow_line(id, text) {
        return Ok(());
    }
    match command.as_str() {
        "/w" | "/m" | "/msg" | "/whisper" => {
            let (to, message) = split(rest);
            if message.is_empty() {
                return usage(server, id, "/w <user> <message>");
            }
            server.hub.lock().whisper(id, &String::from_utf8_lossy(to), message);
        }
        "/r" | "/reply" => {
            let mut hub = server.hub.lock();
            match hub.session(id).and_then(|session| session.last_whisper_from.clone()) {
                Some(name) if !rest.is_empty() => hub.whisper(id, &name, rest),
                _ => hub.send(id, Event::error("No one messaged you, use /m instead")),
            }
        }
        "/me" | "/emote" if rest.is_empty() => return usage(server, id, "/me <text>"),
        "/me" | "/emote" => server.hub.lock().talk(id, EventKind::Emote, rest),
        "/away" => set_away(&mut server.hub.lock(), id, rest),
        "/dnd" => set_busy(&mut server.hub.lock(), id, rest),
        "/whois" | "/where" | "/whereis" if rest.is_empty() => return usage(server, id, "/whois <user>"),
        "/whois" | "/where" | "/whereis" => whois(server, id, &String::from_utf8_lossy(rest)).await?,
        "/whoami" => {
            let name = server.hub.lock().session(id).map(|session| session.name().to_owned());
            whois(server, id, &name.unwrap_or_default()).await?;
        }
        "/who" => who(&server.hub.lock(), id, rest),
        "/join" | "/j" | "/channel" if rest.is_empty() => return usage(server, id, "/join <channel>"),
        "/join" | "/j" | "/channel" if rest.len() > 31 => {
            server.hub.lock().send(id, Event::error("max channel name length exceeded (max 31 symbols)"));
        }
        "/join" | "/j" | "/channel" => server.hub.lock().join_channel(id, rest),
        "/rejoin" => {
            let mut hub = server.hub.lock();
            if let Some(name) = hub.channel_name(id).map(<[u8]>::to_vec) {
                hub.leave_channel(id);
                hub.join_channel(id, &name);
            }
        }
        "/channels" | "/chs" => {
            let hub = server.hub.lock();
            hub.send(id, Event::info("Currently accessible channels:"));
            for name in hub.channel_list(id) {
                hub.send(id, Event::info(name));
            }
        }
        "/games" => {
            let hub = server.hub.lock();
            hub.send(id, Event::info("Currently accessible games:"));
            for game in hub.list_games(id, 0) {
                hub.send(id, Event::info(game.name));
            }
        }
        "/ignore" | "/squelch" | "/unignore" | "/unsquelch" => {
            ignore(&mut server.hub.lock(), id, rest, !command.starts_with("/un"));
        }
        "/stats" => stats(server, id, &String::from_utf8_lossy(rest)).await?,
        "/time" => time(&server.hub.lock(), id),
        "/users" | "/status" | "/lusers" => {
            let hub = server.hub.lock();
            let text =
                format!("There are currently {} users online, in {} games.", hub.user_count(), hub.game_count());
            hub.send(id, Event::info(text));
        }
        "/uptime" => {
            let up = server.uptime().as_secs();
            let text = format!(
                "Uptime: {} days {} hours {} minutes {} seconds",
                up / 86400,
                up / 3600 % 24,
                up / 60 % 60,
                up % 60
            );
            server.hub.lock().send(id, Event::info(text));
        }
        "/latency" | "/ping" | "/p" => {
            let hub = server.hub.lock();
            let latency = hub.session(id).map_or(0, |session| session.latency);
            hub.send(id, Event::info(format!("Your latency {latency}")));
        }
        "/version" => {
            let text =
                concat!("openbnet ", env!("CARGO_PKG_VERSION"), ", free software under the GPL v3 or later");
            server.hub.lock().send(id, Event::info(text));
        }
        "/motd" => {
            let hub = server.hub.lock();
            for line in &server.config().chat.welcome {
                hub.send(id, Event::info(line.as_bytes()));
            }
        }
        "/help" | "/?" => help(&server.hub.lock(), id),
        "/topic" => topic(server, id, rest).await?,
        "/f" | "/friends" => friends(server, id, rest).await?,
        "/kick" | "/ban" | "/unban" | "/op" | "/tmpop" | "/deop" => {
            channel_command(server, id, &command, rest).await?;
        }
        "/announce" | "/ann" => {
            if let Some(name) = require(server, id, Role::Moderator) {
                let mut text = format!("Announcement from {name}: ").into_bytes();
                text.extend_from_slice(rest);
                server.hub.lock().broadcast(&Event::error(text));
            }
        }
        "/ipban" | "/ipunban" | "/ipbans" => ip_bans(server, id, &command, rest).await?,
        "/kill" | "/mute" | "/muteacct" | "/unmute" | "/unmuteacct" | "/lock" | "/lockacct" | "/unlock"
        | "/unlockacct" => account_command(server, id, &command, rest).await?,
        _ => server.hub.lock().send(id, Event::error("Unknown command.")),
    }
    Ok(())
}

fn split(text: &[u8]) -> (&[u8], &[u8]) {
    match text.iter().position(|b| *b == b' ') {
        Some(end) => (&text[..end], text[end..].trim_ascii_start()),
        None => (text, &[]),
    }
}

fn usage(server: &Server, id: SessionId, text: &str) -> Result<()> {
    server.hub.lock().send(id, Event::info(format!("Usage: {text}")));
    Ok(())
}

/// The session's name if it has the role. Otherwise it is told so.
fn require(server: &Server, id: SessionId, role: Role) -> Option<String> {
    let hub = server.hub.lock();
    let session = hub.session(id)?;
    if session.role() < role {
        hub.send(id, Event::error("This command is reserved for admins."));
        return None;
    }
    Some(session.name().to_owned())
}

fn help(hub: &Hub, id: SessionId) {
    for line in HELP {
        hub.send(id, Event::info(*line));
    }
    if hub.is_operator(id) {
        hub.send(id, Event::info(OPERATOR_HELP));
    }
    if hub.session(id).is_some_and(|session| session.role() >= Role::Moderator) {
        hub.send(id, Event::info(MODERATOR_HELP));
    }
}

fn set_away(hub: &mut Hub, id: SessionId, why: &[u8]) {
    let Some(session) = hub.session_mut(id) else { return };
    let text = if why.is_empty() && session.away.take().is_some() {
        "You are no longer marked as away."
    } else {
        session.away = Some(if why.is_empty() { b"Currently not available".to_vec() } else { why.to_vec() });
        "You are now marked as being away."
    };
    hub.send(id, Event::info(text));
}

fn set_busy(hub: &mut Hub, id: SessionId, why: &[u8]) {
    let Some(session) = hub.session_mut(id) else { return };
    let text = if why.is_empty() && session.busy.take().is_some() {
        "Do Not Disturb mode canceled."
    } else {
        session.busy = Some(if why.is_empty() { b"Not available".to_vec() } else { why.to_vec() });
        "Do Not Disturb mode engaged."
    };
    hub.send(id, Event::info(text));
}

fn ignore(hub: &mut Hub, id: SessionId, name: &[u8], ignore: bool) {
    let name = String::from_utf8_lossy(name).trim().to_owned();
    let Some(session) = hub.session_mut(id) else { return };
    let key = name.to_ascii_lowercase();
    const MAX: usize = 100;
    let event = if name.is_empty() {
        Event::info("Usage: /ignore <user>")
    } else if key == session.name().to_ascii_lowercase() {
        Event::error("You can't squelch yourself.")
    } else if ignore && session.ignoring.len() >= MAX {
        Event::error("Could not squelch user.")
    } else if ignore {
        session.ignoring.insert(key);
        Event::info(format!("{name} has been squelched."))
    } else if session.ignoring.remove(&key) {
        Event::info("No longer ignoring.")
    } else {
        Event::info("User was not being ignored.")
    };
    hub.send(id, event);
}

fn who(hub: &Hub, id: SessionId, channel: &[u8]) {
    if channel.is_empty() {
        return hub.send(id, Event::info("Usage: /who <channel>"));
    }
    match hub.channel_members(id, channel) {
        Err(why) => hub.send(id, Event::error(why)),
        Ok(members) => {
            let mut line = format!("Users in channel {}:", String::from_utf8_lossy(channel));
            for member in members {
                if line.len() + member.name.len() + 1 > 200 {
                    hub.send(id, Event::info(std::mem::take(&mut line)));
                }
                line.push(' ');
                line.push_str(&member.name);
            }
            hub.send(id, Event::info(line));
        }
    }
}

fn describe(hub: &Hub, session: &Session, yourself: bool) -> Vec<u8> {
    let (name, verb) = if yourself { ("You", "are") } else { (session.name(), "is") };
    let mut text = format!("{name} {verb} using {}", product::title(session.product)).into_bytes();
    if let Some((game, private)) = hub.game_of(session) {
        let kind = if private { "private " } else { "" };
        text.extend(format!(" and {verb} currently in {kind}game \"").bytes());
        text.extend_from_slice(game);
        text.push(b'"');
    } else if let Some(channel) = hub.channel_name(session.id) {
        text.extend(format!(" and {verb} currently in channel \"").bytes());
        text.extend_from_slice(channel);
        text.push(b'"');
    }
    text.push(b'.');
    text
}

async fn whois(server: &Server, id: SessionId, name: &str) -> Result<()> {
    {
        let hub = server.hub.lock();
        if let Some(found) = hub.find_user(name) {
            hub.send(id, Event::info(describe(&hub, found, found.id == id)));
            let asker_is_staff = hub.session(id).is_some_and(|session| session.role() >= Role::Moderator);
            if asker_is_staff {
                hub.send(id, Event::info(format!("{} is connected from {}", found.name(), found.ip)));
            }
            return Ok(());
        }
    }
    let event = match server.store.account(name).await? {
        None => Event::error("Unknown user."),
        Some(account) if account.last_login_at == 0 => Event::info("User is offline"),
        Some(account) => {
            let bias = server.hub.lock().session(id).map_or(0, |session| session.time_bias_minutes);
            Event::info(format!("User was last seen on: {}", date(account.last_login_at, bias)))
        }
    };
    server.hub.lock().send(id, event);
    Ok(())
}

async fn stats(server: &Server, id: SessionId, name: &str) -> Result<()> {
    let Some((game, own)) = server.hub.lock().session(id).map(|s| (s.product, s.name().to_owned())) else {
        return Ok(());
    };
    let name = if name.is_empty() { &own } else { name };
    let Some(account) = server.store.account(name).await? else {
        server.hub.lock().send(id, Event::error("Invalid user."));
        return Ok(());
    };
    let records = server.store.records(account.id, game).await?;
    let hub = server.hub.lock();
    hub.send(id, Event::info(format!("{}'s record:", account.name)));
    for (ladder, title) in [(0, "Normal games"), (1, "Ladder games"), (3, "Iron Man games")] {
        if let Some(r) = records.get(&ladder).or((ladder == 0).then_some(&Default::default())) {
            let mut text = format!("{title}: {}-{}-{}-{}", r.wins, r.losses, r.draws, r.disconnects);
            if r.rating > 0 {
                text += &format!(", rating {}, rank {}", r.rating, r.rank);
            }
            hub.send(id, Event::info(text));
        }
    }
    Ok(())
}

fn time(hub: &Hub, id: SessionId) {
    let bias = hub.session(id).map_or(0, |session| session.time_bias_minutes);
    hub.send(id, Event::info(format!("Server Time: {} UTC", date(unix_now(), 0))));
    hub.send(id, Event::info(format!("Your local time: {}", date(unix_now(), bias))));
}

/// Formats a time like `Sat Nov 25 18:00:00`, for a client `bias` minutes behind UTC.
fn date(unix: i64, bias: i32) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let local = unix - i64::from(bias.clamp(-24 * 60, 24 * 60)) * 60;
    let (days, seconds) = (local.div_euclid(86400), local.rem_euclid(86400));
    // Howard Hinnant's days-to-civil algorithm, counting years from March.
    let shifted = days + 719_468;
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month + 2) / 5 + 1;
    format!(
        "{} {} {day:02} {:02}:{:02}:{:02}",
        DAYS[days.rem_euclid(7) as usize],
        MONTHS[((month + 2) % 12) as usize],
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

async fn topic(server: &Server, id: SessionId, text: &[u8]) -> Result<()> {
    const MAX: usize = 200;
    let kept = {
        let mut hub = server.hub.lock();
        if hub.channel_name(id).is_none() {
            hub.send(id, Event::error("This command can only be used inside a channel."));
            return Ok(());
        }
        if text.len() > MAX {
            hub.send(id, Event::error("Max topic length exceeded (max 200 symbols)"));
            return Ok(());
        }
        if !text.is_empty() && !hub.is_operator(id) {
            hub.send(id, Event::error("You must be at least a Channel Operator to set the topic"));
            return Ok(());
        }
        let kept = if text.is_empty() { None } else { hub.set_topic(id, text) };
        if let Some(event) = hub.show_topic(id) {
            hub.send(id, event);
        }
        kept
    };
    if let Some(channel) = kept {
        let channel = String::from_utf8_lossy(&channel).into_owned();
        server.store.set_topic(&channel, &String::from_utf8_lossy(text)).await?;
    }
    Ok(())
}

async fn friends(server: &Server, id: SessionId, text: &[u8]) -> Result<()> {
    const MAX: usize = 20;
    let (action, rest) = split(text);
    let name = String::from_utf8_lossy(split(rest).0).into_owned();
    let say = |text: String| server.hub.lock().send(id, Event::info(text));
    let own = {
        let hub = server.hub.lock();
        hub.session(id).and_then(|session| session.account.clone())
    };
    let Some(own) = own else { return Ok(()) };
    let list = server.store.friends(own.id).await?;
    match action {
        b"add" | b"a" if !name.is_empty() => {
            let Some(account) = server.store.account(&name).await? else {
                say("That user does not exist.".into());
                return Ok(());
            };
            if account.id == own.id {
                say("You can't add yourself to your friends list.".into());
            } else if list.len() >= MAX {
                say(format!("You can only have a maximum of {MAX} friends."));
            } else if !server.store.add_friend(own.id, account.id).await? {
                say(format!("{} is already on your friends list!", account.name));
            } else {
                say(format!("Added {} to your friends list.", account.name));
                let list = server.store.friends(own.id).await?;
                if let Some(friend) = list.iter().find(|friend| friend.account == account.id) {
                    bncs::friend_added(server, id, friend);
                }
                let hub = server.hub.lock();
                if let Some(target) = hub.find_user(&account.name) {
                    hub.send(target.id, Event::info(format!("{} added you to their friends list.", own.name)));
                }
            }
        }
        b"remove" | b"r" | b"del" | b"delete" if !name.is_empty() => {
            match list.iter().position(|friend| friend.name.eq_ignore_ascii_case(&name)) {
                None => say(format!("{name} was not found on your friends list.")),
                Some(index) => {
                    server.store.remove_friend(own.id, list[index].account).await?;
                    say(format!("Removed {} from your friends list.", list[index].name));
                    bncs::friend_removed(server, id, index as u8);
                }
            }
        }
        b"msg" | b"m" | b"w" | b"whisper" if !rest.is_empty() => {
            let mut hub = server.hub.lock();
            let mutual = list.iter().filter(|friend| friend.mutual);
            let online: Vec<String> =
                mutual.filter(|friend| hub.is_online(friend.account)).map(|friend| friend.name.clone()).collect();
            if online.is_empty() {
                hub.send(id, Event::info("All of your friends are offline."));
            }
            for name in online {
                hub.whisper(id, &name, rest);
            }
        }
        b"list" | b"l" | b"online" | b"o" => {
            let online_only = action.starts_with(b"o");
            let hub = server.hub.lock();
            hub.send(id, Event::info("Your friends:"));
            for (friend, number) in list.iter().zip(1..) {
                let found = hub.find_user(&friend.name);
                // Only those who are friends back may see where someone is.
                let event = match found {
                    None if online_only => continue,
                    None => Event::info(format!("{number}: {}, offline", friend.name)),
                    Some(found) if friend.mutual => {
                        let mut line = format!("{number}: ").into_bytes();
                        line.extend(describe(&hub, found, false));
                        Event::info(line)
                    }
                    Some(_) => Event::info(format!("{number}: {}, online", friend.name)),
                };
                hub.send(id, event);
            }
        }
        _ => return usage(server, id, "/f add <user>, /f remove <user>, /f list, /f online, /f msg <text>"),
    }
    Ok(())
}

async fn channel_command(server: &Server, id: SessionId, command: &str, rest: &[u8]) -> Result<()> {
    let (name, why) = split(rest);
    let name = String::from_utf8_lossy(name).into_owned();
    let refuse = |text: &str| server.hub.lock().send(id, Event::error(text));
    {
        let hub = server.hub.lock();
        if hub.channel_name(id).is_none() {
            hub.send(id, Event::error("This command can only be used inside a channel."));
            return Ok(());
        }
        if !hub.is_operator(id) {
            let text = "You have to be at least a Channel Operator or tempOP to use this command.";
            hub.send(id, Event::error(text));
            return Ok(());
        }
    }
    if name.is_empty() {
        return usage(server, id, &format!("{command} <user>"));
    }
    let Some(account) = server.store.account(&name).await? else {
        refuse("That account doesn't currently exist.");
        return Ok(());
    };
    let mut hub = server.hub.lock();
    let own_role = hub.session(id).map_or(Role::Player, Session::role);
    if account.role >= Role::Moderator && own_role <= account.role {
        hub.send(id, Event::error("You can't do that to a moderator or administrator."));
        return Ok(());
    }
    let by = hub.session(id).map(|session| session.name().to_owned()).unwrap_or_default();
    let announce = |hub: &Hub, what: &str| {
        let mut text = format!("{} has been {what} by {by}", account.name).into_bytes();
        if !why.is_empty() {
            text.extend_from_slice(b" (");
            text.extend_from_slice(why);
            text.push(b')');
        }
        text.push(b'.');
        hub.announce(id, &Event::info(text));
    };
    let here = hub.channel_name(id).map(<[u8]>::to_vec);
    let target = hub.find_user(&name).map(|session| session.id);
    let in_channel = target.filter(|target| hub.channel_name(*target) == here.as_deref());
    match command {
        "/unban" => {
            let text = if hub.set_banned(id, &account.name, false) {
                format!("{} is no longer banned from this channel.", account.name)
            } else {
                "That user is not banned.".into()
            };
            hub.send(id, Event::info(text));
        }
        "/ban" => {
            hub.set_banned(id, &account.name, true);
            announce(&hub, "banned");
            if let Some(target) = in_channel {
                hub.join_channel(target, crate::hub::VOID.as_bytes());
            }
        }
        _ => {
            let Some(target) = in_channel else {
                hub.send(id, Event::error("That user is not in this channel."));
                return Ok(());
            };
            match command {
                "/kick" => {
                    announce(&hub, "kicked");
                    hub.eject(target);
                }
                "/deop" => {
                    hub.set_operator(id, target, false);
                }
                _ => {
                    hub.set_operator(id, target, true);
                }
            }
        }
    }
    Ok(())
}

async fn account_command(server: &Server, id: SessionId, command: &str, rest: &[u8]) -> Result<()> {
    let Some(_) = require(server, id, Role::Moderator) else { return Ok(()) };
    let name = String::from_utf8_lossy(split(rest).0).into_owned();
    let Some(account) = server.store.account(&name).await? else {
        server.hub.lock().send(id, Event::error("Invalid user."));
        return Ok(());
    };
    let own_role = server.hub.lock().session(id).map_or(Role::Player, Session::role);
    if account.role >= own_role {
        server.hub.lock().send(id, Event::error("You can't do that to someone of your own rank or above."));
        return Ok(());
    }
    let done = match command {
        "/kill" => {
            disconnect(server, &account);
            "disconnected"
        }
        "/lock" | "/lockacct" => {
            server.store.set_locked(account.id, true).await?;
            disconnect(server, &account);
            "locked"
        }
        "/unlock" | "/unlockacct" => {
            server.store.set_locked(account.id, false).await?;
            "unlocked"
        }
        "/mute" | "/muteacct" => {
            server.store.set_muted(account.id, true).await?;
            server.hub.lock().set_muted(account.id, true);
            "muted"
        }
        _ => {
            server.store.set_muted(account.id, false).await?;
            server.hub.lock().set_muted(account.id, false);
            "unmuted"
        }
    };
    tracing::info!(moderator = id, account = account.name, done, "moderation");
    server.hub.lock().send(id, Event::info(format!("{} is now {done}.", account.name)));
    Ok(())
}

async fn ip_bans(server: &Server, id: SessionId, command: &str, rest: &[u8]) -> Result<()> {
    let Some(_) = require(server, id, Role::Moderator) else { return Ok(()) };
    let (target, rest) = split(rest);
    let target = String::from_utf8_lossy(target).into_owned();
    let say = |event: Event| server.hub.lock().send(id, event);
    match command {
        "/ipbans" => {
            let hub = server.hub.lock();
            hub.send(id, Event::info(format!("{} bans:", hub.bans().len())));
            for ban in hub.bans() {
                let until =
                    if ban.until == 0 { "for good".into() } else { format!("until {} UTC", date(ban.until, 0)) };
                hub.send(id, Event::info(format!("{} {until}, by {} {}", ban.range, ban.by, ban.reason)));
            }
        }
        "/ipunban" => match target.parse() {
            Err(_) => return usage(server, id, "/ipunban <address>"),
            Ok(range) if ban::remove(server, range).await? => {
                say(Event::info(format!("{range} is no longer banned.")))
            }
            Ok(range) => say(Event::error(format!("{range} is not banned."))),
        },
        _ if target.is_empty() => return usage(server, id, "/ipban <user or address> [hours] [why]"),
        _ => {
            let (first, others) = split(rest);
            let hours = std::str::from_utf8(first).ok().and_then(|text| text.parse::<u32>().ok());
            let reason = String::from_utf8_lossy(if hours.is_some() { others } else { rest }).into_owned();
            say(match ban::add(server, Some(id), &target, hours.unwrap_or(0), &reason).await? {
                Ok(range) => Event::info(format!("{range} is now banned.")),
                Err(why) => Event::error(why),
            });
        }
    }
    Ok(())
}

fn disconnect(server: &Server, account: &Account) {
    server.hub.lock().disconnect_account(account.id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_right_across_month_and_leap_boundaries() {
        assert_eq!(date(0, 0), "Thu Jan 01 00:00:00");
        assert_eq!(date(951_782_400, 0), "Tue Feb 29 00:00:00");
        assert_eq!(date(1_709_251_199, 0), "Thu Feb 29 23:59:59");
        assert_eq!(date(0, 360), "Wed Dec 31 18:00:00");
    }

    #[test]
    fn splits_a_command_from_its_text() {
        assert_eq!(split(b"/w  bob hi there"), (&b"/w"[..], &b"bob hi there"[..]));
        assert_eq!(split(b"/help"), (&b"/help"[..], &b""[..]));
    }
}
