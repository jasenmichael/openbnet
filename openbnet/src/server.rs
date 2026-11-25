//! Accepts connections and ties the parts together.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use parking_lot::{Mutex, RwLock};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};
use tokio::sync::{Semaphore, mpsc};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::config::Config;
use crate::files::{self, Files};
use crate::hub::Hub;
use crate::store::{FinishedGame, Store};
use crate::text::{self, Style};
use crate::versions::Versions;
use crate::{bncs, http, irc, udp};

pub struct Server {
    config: RwLock<Arc<Config>>,
    /// The data folder, to read the settings again.
    data: Option<PathBuf>,
    pub store: Store,
    pub hub: Mutex<Hub>,
    pub files: Files,
    pub versions: Versions,
    pub udp: UdpSocket,
    started: Instant,
}

/// The first byte a client sends says which protocol it speaks.
mod protocol {
    pub const GAME: u8 = 0x01;
    pub const FILES: u8 = 0x02;
    pub const BOT: u8 = 0x03;
    /// Someone at a terminal pressing return.
    pub const TELNET: u8 = 0x0d;
}

impl Server {
    pub fn config(&self) -> Arc<Config> {
        self.config.read().clone()
    }

    /// Reads the settings again. Ports, the database and the files directory need a restart.
    pub fn reload(&self) -> Result<()> {
        let path = self.data.as_ref().context("the server was started without a data folder")?;
        self.reconfigure(Config::load(path)?);
        Ok(())
    }

    pub fn reconfigure(&self, config: Config) {
        self.hub.lock().reconfigure(&config);
        *self.config.write() = Arc::new(config);
        tracing::info!("settings reloaded");
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// Milliseconds since the server started, for measuring latency.
    pub fn ticks(&self) -> u32 {
        self.started.elapsed().as_millis() as u32
    }
}

pub struct Running {
    pub server: Arc<Server>,
    pub address: SocketAddr,
    listener: TcpListener,
    pub irc: Option<TcpListener>,
    pub irc_tls: Option<(TcpListener, TlsAcceptor)>,
    pub http: Option<TcpListener>,
    finished: mpsc::UnboundedReceiver<FinishedGame>,
}

/// Opens the database and the ports.
pub async fn start(config: Config, data: Option<PathBuf>) -> Result<Running> {
    let store = Store::open(&config.database.url).await.context("opening the database")?;
    let listener =
        listen(config.server.listen).with_context(|| format!("listening on {}", config.server.listen))?;
    let address = listener.local_addr()?;
    let udp = UdpSocket::bind(address).await.with_context(|| format!("listening on UDP {address}"))?;
    let (sender, finished) = mpsc::unbounded_channel();
    let server = Arc::new(Server {
        hub: Mutex::new(Hub::new(&config, sender)),
        files: Files::new(&config.files),
        versions: Versions::load()?,
        store,
        udp,
        started: Instant::now(),
        config: RwLock::new(Arc::new(config)),
        data,
    });
    for (channel, topic) in server.store.topics().await? {
        server.hub.lock().restore_topic(&channel, &topic);
    }
    let irc = listen_if(server.config().irc.listen)?;
    let irc_tls = listen_tls(&server.config().irc)?;
    let http = listen_if(server.config().http.listen)?;
    Ok(Running { server, address, listener, irc, irc_tls, http, finished })
}

/// The usual queue of 128 overflows when many players reconnect at once, as after a restart.
fn listen(address: SocketAddr) -> std::io::Result<TcpListener> {
    let socket = if address.is_ipv4() { TcpSocket::new_v4()? } else { TcpSocket::new_v6()? };
    socket.set_reuseaddr(true)?;
    socket.bind(address)?;
    socket.listen(4096)
}

fn listen_if(address: Option<SocketAddr>) -> Result<Option<TcpListener>> {
    address.map(|address| listen(address).with_context(|| format!("listening on {address}"))).transpose()
}

fn listen_tls(config: &crate::config::Irc) -> Result<Option<(TcpListener, TlsAcceptor)>> {
    let Some(address) = config.tls_listen else { return Ok(None) };
    let certificate = config.certificate.as_ref().context("irc.tls_listen needs irc.certificate")?;
    let key = config.key.as_ref().context("irc.tls_listen needs irc.key")?;
    let certificates = CertificateDer::pem_file_iter(certificate)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .with_context(|| format!("reading {}", certificate.display()))?;
    let key = PrivateKeyDer::from_pem_file(key).with_context(|| format!("reading {}", key.display()))?;
    let tls = ServerConfig::builder().with_no_client_auth().with_single_cert(certificates, key)?;
    let listener = listen(address).with_context(|| format!("listening on {address}"))?;
    Ok(Some((listener, TlsAcceptor::from(Arc::new(tls)))))
}

impl Running {
    /// Serves until the task is dropped.
    pub async fn run(self) {
        let Running { server, listener, irc, irc_tls, http, finished, .. } = self;
        // One limit for every protocol, as each connection costs about the same.
        let slots = Arc::new(Semaphore::new(server.config().server.max_connections));
        let games = {
            let server = server.clone();
            accept(listener, slots.clone(), move |stream, ip| game_port(server.clone(), stream, ip))
        };
        let irc = async {
            let Some(listener) = irc else { return };
            let server = server.clone();
            accept(listener, slots.clone(), move |stream, ip| irc::serve(server.clone(), stream, ip)).await;
        };
        let irc_tls = async {
            let Some((listener, tls)) = irc_tls else { return };
            let server = server.clone();
            accept(listener, slots.clone(), move |stream, ip| {
                let (server, tls) = (server.clone(), tls.clone());
                async move {
                    let stream = tokio::time::timeout(Duration::from_secs(30), tls.accept(stream)).await??;
                    irc::serve(server, stream, ip).await
                }
            })
            .await;
        };
        let http = async {
            let Some(listener) = http else { return };
            let app = http::router(server.clone()).into_make_service_with_connect_info::<SocketAddr>();
            if let Err(error) = axum::serve(listener, app).await {
                tracing::error!(%error, "the API stopped");
            }
        };
        tokio::join!(
            games,
            irc,
            irc_tls,
            http,
            udp::serve(server.clone()),
            record_games(server.clone(), finished)
        );
    }
}

async fn accept<F, Fut>(listener: TcpListener, slots: Arc<Semaphore>, serve: F)
where
    F: Fn(TcpStream, IpAddr) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(error) => {
                tracing::warn!(%error, "could not accept a connection");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            tracing::warn!(%peer, "refused: too many connections");
            continue;
        };
        if stream.set_nodelay(true).is_err() {
            continue;
        }
        let serving = serve(stream, peer.ip().to_canonical());
        tokio::spawn(async move {
            if let Err(error) = serving.await {
                tracing::debug!(%peer, %error, "connection ended");
            }
            drop(slot);
        });
    }
}

async fn game_port(server: Arc<Server>, mut stream: TcpStream, ip: IpAddr) -> Result<()> {
    let config = server.config();
    let limit = Duration::from_secs(config.server.login_timeout_secs);
    match (tokio::time::timeout(limit, stream.read_u8()).await??, ip) {
        // Game hosts are sent to players as IPv4, so that is all games may use.
        (protocol::GAME, IpAddr::V4(ip)) => bncs::serve(server, stream, ip).await,
        (protocol::FILES, _) => tokio::time::timeout(limit, files::serve(&server.files, &mut stream)).await?,
        (protocol::BOT, _) if config.bots.enabled => text::serve(server, stream, ip, Style::Bot).await,
        (protocol::TELNET, _) if config.bots.enabled => text::serve(server, stream, ip, Style::Telnet).await,
        (other, _) => anyhow::bail!("protocol {other:#04x} not available"),
    }
}

async fn record_games(server: Arc<Server>, mut finished: mpsc::UnboundedReceiver<FinishedGame>) {
    while let Some(game) = finished.recv().await {
        if let Err(error) = record_game(&server, &game).await {
            tracing::error!(%error, name = game.name, "could not store a game's result");
        }
    }
}

async fn record_game(server: &Server, game: &FinishedGame) -> Result<()> {
    server.store.finish_game(game).await?;
    for (account, _) in &game.results {
        let records = server.store.records(*account, game.product).await?;
        server.hub.lock().set_records(*account, game.product, records);
    }
    Ok(())
}
