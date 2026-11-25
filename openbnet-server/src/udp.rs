//! UDP, through which the server learns where a player's games can be reached.

use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;

use crate::server::Server;
use crate::wire::{Reader, Result, Writer};

const TEST: u32 = 0x05;
const ADDRESS: u32 = 0x08;
const ADDRESS_WITH_ID: u32 = 0x09;

pub async fn serve(server: Arc<Server>) {
    let mut buffer = [0; 512];
    loop {
        match server.udp.recv_from(&mut buffer).await {
            Ok((size, SocketAddr::V4(from))) => {
                if note_address(&server, &buffer[..size], from).is_ok_and(|known| known) {
                    send_test(&server, from).await;
                }
            }
            Ok(_) => {}
            // Windows and Linux report earlier sends' failures here. They don't affect this socket.
            Err(error) => tracing::debug!(%error, "UDP receive failed"),
        }
    }
}

/// True if the packet came from a connected player.
fn note_address(server: &Server, packet: &[u8], from: SocketAddrV4) -> Result<bool> {
    let mut r = Reader::new(packet);
    let kind = r.u32()?;
    if kind != ADDRESS && kind != ADDRESS_WITH_ID {
        return Ok(false);
    }
    let token = r.u32()?;
    let id = if kind == ADDRESS_WITH_ID { Some(r.u32()?) } else { None };
    let mut hub = server.hub.lock();
    let session = match id {
        Some(id) => hub.session_mut(id).filter(|session| session.token == token),
        None => hub.session_by_token(token),
    };
    let Some(session) = session else { return Ok(false) };
    session.game_address = from;
    Ok(true)
}

/// Sends the packets a client answers to show that its UDP works.
pub async fn send_test(server: &Server, to: SocketAddrV4) {
    let packet = Writer::new().u32(TEST).raw(b"tenb").finish();
    for _ in 0..2 {
        if let Err(error) = server.udp.send_to(&packet, to).await {
            tracing::debug!(%error, %to, "UDP send failed");
        }
    }
}
