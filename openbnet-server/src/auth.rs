//! Signing up and logging in, whatever the protocol.

use std::net::IpAddr;

use anyhow::Result;

use crate::hub::SessionId;
use crate::server::Server;
use crate::store::{Account, Created};
use crate::wire::Tag;
use crate::xsha1::Hash;

pub const MIN_NAME: usize = 2;
pub const MAX_NAME: usize = 15;
pub const MAX_PASSWORD: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginError {
    NoAccount,
    BadPassword,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignupError {
    InvalidName,
    NameTaken,
    NotAllowed,
}

/// Finds an account if `matches` accepts its stored password. An account without a usable one can't be entered.
pub async fn check(
    server: &Server,
    ip: IpAddr,
    name: &str,
    matches: impl FnOnce(Hash) -> bool,
) -> Result<Result<Account, LoginError>> {
    if server.hub.lock().too_many_failures(ip) {
        return Ok(Err(LoginError::BadPassword));
    }
    let account = server.store.account(name).await?;
    let stored = match &account {
        Some(account) => server.store.password(account.id).await?,
        None => None,
    };
    let correct = stored.is_some_and(matches);
    if !correct {
        server.hub.lock().note_failure(ip);
    }
    Ok(match account {
        None => Err(LoginError::NoAccount),
        Some(_) if !correct => Err(LoginError::BadPassword),
        Some(account) if account.locked => Err(LoginError::Locked),
        Some(account) => Ok(account),
    })
}

/// The above, for protocols that send the password itself.
pub async fn check_password(
    server: &Server,
    ip: IpAddr,
    name: &str,
    password: &str,
) -> Result<Result<Account, LoginError>> {
    let hash = Hash::of_password(password);
    check(server, ip, name, |stored| stored.matches(&hash)).await
}

/// Puts a session in its account. False means the session has gone.
pub async fn enter(server: &Server, id: SessionId, ip: IpAddr, account: Account, product: Tag) -> Result<bool> {
    let records = server.store.records(account.id, product).await?;
    server.store.note_login(account.id, product, &ip.to_string()).await?;
    tracing::info!(session = id, name = account.name, %product, "logged in");
    Ok(server.hub.lock().log_in(id, account, records))
}

pub async fn sign_up(server: &Server, ip: IpAddr, name: &[u8], password: Hash) -> Result<Result<(), SignupError>> {
    let config = server.config();
    let Some(name) = valid_name(name, &config.accounts.symbols) else {
        return Ok(Err(SignupError::InvalidName));
    };
    if !config.accounts.allow_new || server.hub.lock().too_many_signups(ip) {
        return Ok(Err(SignupError::NotAllowed));
    }
    let created = server.store.create_account(name, password).await?;
    tracing::info!(name, ?created, "account requested");
    if created == Created::NameTaken {
        return Ok(Err(SignupError::NameTaken));
    }
    server.hub.lock().note_signup(ip);
    Ok(Ok(()))
}

/// A name that may be registered: 2 to 15 letters, digits and allowed symbols.
pub fn valid_name<'a>(name: &'a [u8], symbols: &str) -> Option<&'a str> {
    let name = std::str::from_utf8(name).ok()?;
    let allowed = |c: char| {
        c.is_ascii_alphanumeric() || (c.is_ascii_graphic() && !matches!(c, '/' | '\\') && symbols.contains(c))
    };
    ((MIN_NAME..=MAX_NAME).contains(&name.len()) && name.chars().all(allowed)).then_some(name)
}
