use std::io::BufRead;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use openbnet::auth::valid_name;
use openbnet::bncs;
use openbnet::config::Config;
use openbnet::server;
use openbnet::store::{Created, Role, Store};
use openbnet::wire::product::title;
use openbnet::xsha1::Hash;
use tokio::signal::unix::{SignalKind, signal};

/// A Battle.net-compatible server for Blizzard's classic games.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Folder for the settings, the database and the games' files. Created if needed.
    #[arg(long, global = true, env = "OPENBNET_DATA", default_value = "data")]
    data: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server.
    Serve,
    /// Create an account. The password is read from standard input.
    CreateAccount {
        name: String,
        /// player, moderator or admin
        #[arg(long, default_value = "player")]
        role: String,
    },
    /// Change an account's role. It applies from the player's next login.
    SetRole { name: String, role: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "openbnet=info,warn".into());
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).init();
    let args = Args::parse();
    let config = Config::open(&args.data)?;
    match args.command {
        Command::Serve => serve(config, args.data).await,
        Command::CreateAccount { name, role } => {
            let role = parse_role(&role)?;
            if valid_name(name.as_bytes(), &config.accounts.symbols).is_none() {
                bail!("names are 2 to 15 letters, digits and {}", config.accounts.symbols);
            }
            eprintln!("Password:");
            let mut password = String::new();
            std::io::stdin().lock().read_line(&mut password)?;
            let password = password.trim_end_matches(['\r', '\n']);
            if password.is_empty() {
                bail!("the password is empty");
            }
            let store = Store::open(&config.database.url).await?;
            match store.create_account(&name, Hash::of_password(password)).await? {
                Created::NameTaken => bail!("{name} already exists"),
                Created::Yes(id) => store.set_role(id, role).await,
            }
        }
        Command::SetRole { name, role } => {
            let store = Store::open(&config.database.url).await?;
            let account = store.account(&name).await?.with_context(|| format!("no account named {name}"))?;
            store.set_role(account.id, parse_role(&role)?).await
        }
    }
}

fn parse_role(text: &str) -> Result<Role> {
    Role::parse(text).context("a role is player, moderator or admin")
}

/// Without its version-check files a game can't log in, and its own message doesn't say why.
async fn warn_about_missing_files(server: &server::Server) {
    let config = server.config();
    let directory = config.files.directory.display();
    for game in bncs::SUPPORTED {
        if !config.server.products.is_empty() && !config.server.products.contains(&game.to_string()) {
            continue;
        }
        let files = server.versions.files(*game);
        let mut available = files.is_empty();
        for file in &files {
            available |= server.files.read(file).await.is_some();
        }
        if !available {
            let files = files.into_iter().collect::<Vec<_>>().join(" or ");
            tracing::warn!("{} can't log in: no {files} in {directory}/ (see README.md)", title(*game));
        }
    }
}

async fn serve(config: Config, data: PathBuf) -> Result<()> {
    let running = server::start(config, Some(data)).await?;
    let server = running.server.clone();
    tracing::info!(address = %running.address, "listening");
    warn_about_missing_files(&server).await;
    let mut hangup = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let running = running.run();
    tokio::pin!(running);
    loop {
        tokio::select! {
            () = &mut running => return Ok(()),
            result = tokio::signal::ctrl_c() => return Ok(result?),
            _ = terminate.recv() => return Ok(()),
            _ = hangup.recv() => {
                if let Err(error) = server.reload() {
                    tracing::error!(error = format!("{error:#}"), "settings not reloaded");
                }
            }
        }
    }
}
