// SPDX-License-Identifier: GPL-3.0-or-later
//
// Translated from PvPGN's src/common/bnethash.cpp in 2026:
// Copyright (C) 1999  Descolada (dyn1-tnt9-237.chicago.il.ameritech.net)
// Copyright (C) 1999,2000  Ross Combs (rocombs@cs.nmsu.edu)
// PvPGN is under the GNU General Public License, version 2 or any later version.

//! Blizzard's variant of SHA-1, used by every classic game except Warcraft III.
//!
//! Ported from PvPGN's `common/bnethash.cpp`. It differs from SHA-1 in byte order, in having no length
//! padding, and in rotating the constant 1 by a data-dependent amount when expanding the block.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Hash(pub [u32; 5]);

impl Hash {
    pub fn of(data: &[u8]) -> Hash {
        let mut state = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0];
        for block in data.chunks(64) {
            compress(&mut state, block);
        }
        Hash(state)
    }

    /// The proof sent at login: the stored hash, hashed again with both sides' tokens.
    pub fn proof(&self, client_token: u32, server_token: u32) -> Hash {
        let mut data = Vec::with_capacity(28);
        data.extend_from_slice(&client_token.to_le_bytes());
        data.extend_from_slice(&server_token.to_le_bytes());
        data.extend_from_slice(&self.to_wire());
        Hash::of(&data)
    }

    /// The hash of a password as the games compute it. They lower-case it first.
    pub fn of_password(password: &str) -> Hash {
        Hash::of(password.to_ascii_lowercase().as_bytes())
    }

    pub fn to_wire(self) -> [u8; 20] {
        let mut out = [0; 20];
        for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.0) {
            *chunk = word.to_le_bytes();
        }
        out
    }

    pub fn from_wire(bytes: [u8; 20]) -> Hash {
        let mut words = [0; 5];
        for (word, chunk) in words.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *word = u32::from_le_bytes(*chunk);
        }
        Hash(words)
    }

    pub fn parse(text: &str) -> Option<Hash> {
        if text.len() != 40 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut words = [0; 5];
        for (i, word) in words.iter_mut().enumerate() {
            *word = u32::from_str_radix(&text[i * 8..i * 8 + 8], 16).ok()?;
        }
        Some(Hash(words))
    }

    /// Compares without stopping at the first difference.
    pub fn matches(&self, other: &Hash) -> bool {
        self.0.iter().zip(other.0).fold(0, |diff, (a, b)| diff | (a ^ b)) == 0
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|word| write!(f, "{word:08x}"))
    }
}

// A hash is enough to log in with, so keep it out of logs.
impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Hash(..)")
    }
}

fn compress(state: &mut [u32; 5], block: &[u8]) {
    let mut w = [0u32; 80];
    for (i, byte) in block.iter().enumerate() {
        w[i / 4] |= u32::from(*byte) << (8 * (i % 4));
    }
    for i in 0..64 {
        w[i + 16] = 1u32.rotate_left((w[i] ^ w[i + 8] ^ w[i + 2] ^ w[i + 13]) & 31);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (i, word) in w.iter().enumerate() {
        let (f, k) = match i / 20 {
            0 => ((b & c) | (!b & d), 0x5a82_7999),
            1 => (b ^ c ^ d, 0x6ed9_eba1),
            2 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
            _ => (b ^ c ^ d, 0xca62_c1d6),
        };
        let next = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*word);
        (e, d, c, b, a) = (d, c, b.rotate_left(30), a, next);
    }
    for (word, value) in state.iter_mut().zip([a, b, c, d, e]) {
        *word = word.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_pvpgn_bnpass() {
        // Vectors produced by PvPGN's own `bnpass` tool.
        for (password, expected) in VECTORS {
            assert_eq!(Hash::of(password.as_bytes()).to_string(), *expected, "{password}");
        }
    }

    #[test]
    fn survives_text_and_wire_forms() {
        let hash = Hash::of(b"secret");
        assert_eq!(Hash::parse(&hash.to_string()), Some(hash));
        assert_eq!(Hash::from_wire(hash.to_wire()), hash);
        assert_eq!(Hash::parse("zz"), None);
    }

    const VECTORS: &[(&str, &str)] = &[
        ("password", "1d0dc8ecc058e776258cdab9ff6a10ff1629248e"),
        ("openbnet123", "c72a611c82368f366cbc8d8197dba5d8e72b020c"),
        (
            "0123456789012345678901234567890123456789012345678901234567890123456789abc",
            "79436bd8c827c6efdedc3568fc39d510ed7ea0f4",
        ),
    ];
}
