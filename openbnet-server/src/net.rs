//! What every chat connection needs, whatever its protocol.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::hub::{Event, Out, Outbox, SessionId};
use crate::server::Server;
use crate::wire::Tag;

const QUEUE: usize = 1024;

/// Turns an event into a protocol's bytes. Empty means it isn't shown.
pub type Encoder = Box<dyn FnMut(&Event) -> Vec<u8> + Send>;

/// A session that ends when this does, however that happens.
pub struct Registration {
    server: Arc<Server>,
    pub id: SessionId,
    pub outbox: Outbox,
}

impl Registration {
    pub fn new(server: &Arc<Server>, ip: IpAddr, product: Tag) -> Result<(Registration, mpsc::Receiver<Out>)> {
        let (queue, outgoing) = mpsc::channel(QUEUE);
        let outbox = Outbox { queue, close: CancellationToken::new() };
        let Some(id) = server.hub.lock().connect(ip, product, outbox.clone()) else {
            anyhow::bail!("refused: banned, too many connections, or stopping");
        };
        Ok((Registration { server: server.clone(), id, outbox }, outgoing))
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.server.hub.lock().disconnect(self.id);
        self.outbox.close.cancel();
    }
}

/// A registered connection and the task that writes to it.
pub struct Link {
    pub id: SessionId,
    pub outbox: Outbox,
    registration: Registration,
    writing: JoinHandle<()>,
}

impl Link {
    pub fn open<W: AsyncWrite + Unpin + Send + 'static>(
        server: &Arc<Server>,
        ip: IpAddr,
        product: Tag,
        writer: W,
        encode: Encoder,
    ) -> Result<Link> {
        let (registration, outgoing) = Registration::new(server, ip, product)?;
        let outbox = registration.outbox.clone();
        let writing = tokio::spawn(write(writer, outgoing, outbox.close.clone(), encode));
        Ok(Link { id: registration.id, outbox, registration, writing })
    }

    /// Runs a connection until `reading` ends. It must end when the outbox closes, but only between requests:
    /// abandoning a database query costs the database connection.
    pub async fn run(self, reading: impl Future<Output = Result<()>>) -> Result<()> {
        let result = reading.await;
        drop(self.registration);
        self.writing.await?;
        result
    }
}

async fn write<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut queue: mpsc::Receiver<Out>,
    close: CancellationToken,
    mut encode: Encoder,
) {
    const LIMIT: Duration = Duration::from_secs(30);
    let mut send = async |writer: &mut W, out| {
        let data = match out {
            Out::Event(event) => encode(&event),
            Out::Raw(data) => data,
        };
        let sending = async {
            writer.write_all(&data).await?;
            writer.flush().await
        };
        matches!(tokio::time::timeout(LIMIT, sending).await, Ok(Ok(())))
    };
    loop {
        tokio::select! {
            biased;
            () = close.cancelled() => break,
            out = queue.recv() => {
                let Some(out) = out else { break };
                if !send(&mut writer, out).await {
                    close.cancel();
                    return;
                }
            }
        }
    }
    // What was already queued may say why the connection is closing.
    while let Ok(out) = queue.try_recv() {
        if !send(&mut writer, out).await {
            return;
        }
    }
    let _ = writer.shutdown().await;
}

/// Splits a stream into lines ended by CR, LF or both. Empty lines are skipped.
pub struct Lines<R> {
    reader: R,
    buffer: Vec<u8>,
    max: usize,
}

impl<R: AsyncRead + Unpin> Lines<R> {
    pub fn new(reader: R, max: usize) -> Self {
        Lines { reader, buffer: Vec::new(), max }
    }

    /// The next line, or None when the stream ends. Can be cancelled and called again without losing data.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        loop {
            while let Some(end) = self.buffer.iter().position(|b| matches!(b, b'\r' | b'\n')) {
                let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
                line.pop();
                line.retain(|b| *b != 0);
                if !line.is_empty() {
                    line.truncate(self.max);
                    return Ok(Some(line));
                }
            }
            anyhow::ensure!(self.buffer.len() <= self.max, "line too long");
            if self.reader.read_buf(&mut self.buffer).await? == 0 {
                return Ok(None);
            }
        }
    }
}

/// Text from one protocol could otherwise add lines to another.
pub fn one_line(text: &[u8]) -> Vec<u8> {
    text.iter().map(|b| if matches!(b, b'\r' | b'\n' | 0) { b' ' } else { *b }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::wire::product;

    #[tokio::test]
    async fn a_crash_ends_only_its_own_session() {
        let mut config = Config::default();
        config.server.listen = "127.0.0.1:0".parse().unwrap();
        config.database.url = "sqlite::memory:".into();
        config.http.listen = None;
        let server = crate::server::start(config, None).await.unwrap().server;
        let ip = "192.0.2.1".parse().unwrap();
        let open = || Link::open(&server, ip, product::CHAT, tokio::io::sink(), Box::new(|_| Vec::new())).unwrap();
        let other = open();
        let crashing = open();
        let crash = async { panic!("deliberate, for this test") };
        assert!(tokio::spawn(crashing.run(crash)).await.unwrap_err().is_panic());
        let left: Vec<_> = server.hub.lock().sessions().map(|session| session.id).collect();
        assert_eq!(left, [other.id]);
    }

    #[tokio::test]
    async fn lines_end_any_way_and_cannot_grow_for_ever() {
        let mut lines = Lines::new(&b"one\r\n\r\ntwo\nthree\rrest"[..], 10);
        for expected in [Some(b"one".to_vec()), Some(b"two".to_vec()), Some(b"three".to_vec()), None] {
            assert_eq!(lines.next().await.unwrap(), expected);
        }
        assert!(Lines::new(&[b'x'; 100][..], 10).next().await.is_err());
    }
}
