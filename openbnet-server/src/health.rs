//! Asking a running server whether it answers.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::{Result, ensure};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::config::Config;

/// Connects as a game would and has an empty packet answered.
pub async fn check(config: &Config) -> Result<()> {
    let mut address = config.server.listen;
    if address.ip().is_unspecified() {
        address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), address.port());
    }
    let asking = async {
        let mut stream = TcpStream::connect(address).await?;
        stream.write_all(b"\x01\xff\x00\x04\x00").await?;
        let mut answer = [0; 4];
        stream.read_exact(&mut answer).await?;
        ensure!(answer == *b"\xff\x00\x04\x00", "an unexpected answer");
        Ok(())
    };
    tokio::time::timeout(Duration::from_secs(5), asking).await?
}
