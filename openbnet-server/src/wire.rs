//! Reading and writing the little-endian, NUL-terminated data the games use.

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed packet")
    }
}

impl std::error::Error for Malformed {}

pub type Result<T> = std::result::Result<T, Malformed>;

/// Reads a packet from start to end. Nothing it does can panic on hostile input.
#[derive(Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data }
    }

    pub fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let (head, tail) = self.data.split_first_chunk().ok_or(Malformed)?;
        self.data = tail;
        Ok(*head)
    }

    pub fn u16(&mut self) -> Result<u16> {
        self.bytes().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32> {
        self.bytes().map(u32::from_le_bytes)
    }

    pub fn skip(&mut self, count: usize) -> Result<()> {
        self.data = self.data.get(count..).ok_or(Malformed)?;
        Ok(())
    }

    /// A NUL-terminated string of at most `max` bytes, without its terminator.
    pub fn cstr(&mut self, max: usize) -> Result<&'a [u8]> {
        let end = self.data.iter().position(|b| *b == 0).ok_or(Malformed)?;
        if end > max {
            return Err(Malformed);
        }
        let text = &self.data[..end];
        self.data = &self.data[end + 1..];
        Ok(text)
    }

    /// Like `cstr`, for text that must be ASCII such as account names and file names.
    pub fn ascii(&mut self, max: usize) -> Result<&'a str> {
        let text = self.cstr(max)?;
        if !text.is_ascii() {
            return Err(Malformed);
        }
        std::str::from_utf8(text).map_err(|_| Malformed)
    }
}

/// Builds a packet.
#[derive(Default)]
pub struct Writer(Vec<u8>);

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn u16(mut self, value: u16) -> Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u32(mut self, value: u32) -> Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u64(mut self, value: u64) -> Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn raw(mut self, data: impl AsRef<[u8]>) -> Self {
        self.0.extend_from_slice(data.as_ref());
        self
    }

    /// Appends a string and its terminator. Embedded NULs would shift every later field, so they end it.
    pub fn cstr(mut self, text: impl AsRef<[u8]>) -> Self {
        let text = text.as_ref();
        let end = text.iter().position(|b| *b == 0).unwrap_or(text.len());
        self.0.extend_from_slice(&text[..end]);
        self.0.push(0);
        self
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// A four-character code such as `W2BN` or `IX86`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Tag(pub u32);

impl Tag {
    pub const fn new(text: &[u8; 4]) -> Tag {
        Tag(u32::from_be_bytes(*text))
    }

    pub fn parse(text: &str) -> Option<Tag> {
        text.as_bytes().try_into().ok().map(|bytes: [u8; 4]| Tag(u32::from_be_bytes(bytes)))
    }

    /// The code as it appears in memory on the wire, e.g. `NB2W`, which is how statstrings spell it.
    pub fn reversed(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.to_be_bytes() {
            write!(f, "{}", if byte.is_ascii_graphic() { byte as char } else { '?' })?;
        }
        Ok(())
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

pub mod product {
    use super::Tag;

    pub const WARCRAFT2: Tag = Tag::new(b"W2BN");
    pub const STARCRAFT: Tag = Tag::new(b"STAR");
    pub const BROOD_WAR: Tag = Tag::new(b"SEXP");
    pub const STARCRAFT_SHAREWARE: Tag = Tag::new(b"SSHR");
    pub const STARCRAFT_JAPAN: Tag = Tag::new(b"JSTR");
    pub const DIABLO: Tag = Tag::new(b"DRTL");
    pub const DIABLO_SHAREWARE: Tag = Tag::new(b"DSHR");
    pub const DIABLO2: Tag = Tag::new(b"D2DV");
    pub const DIABLO2_EXPANSION: Tag = Tag::new(b"D2XP");
    pub const CHAT: Tag = Tag::new(b"CHAT");
    pub const IRC: Tag = Tag::new(b"IIRC");
    pub const WEB: Tag = Tag::new(b"WEBC");

    pub fn title(tag: Tag) -> &'static str {
        match tag {
            WARCRAFT2 => "Warcraft II",
            STARCRAFT => "Starcraft",
            BROOD_WAR => "Starcraft: Brood War",
            STARCRAFT_SHAREWARE => "Starcraft (Shareware)",
            STARCRAFT_JAPAN => "Starcraft (Japan)",
            DIABLO => "Diablo I",
            DIABLO_SHAREWARE => "Diablo I (Shareware)",
            DIABLO2 => "Diablo II",
            DIABLO2_EXPANSION => "Diablo II Lord of Destruction",
            CHAT => "Chat",
            IRC => "Internet Relay Chat",
            WEB => "Web Chat",
            _ => "an unknown game",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_in_order_and_rejects_short_data() {
        let data = Writer::new().u32(7).cstr("name").u16(9).finish();
        let mut r = Reader::new(&data);
        assert_eq!(r.u32(), Ok(7));
        assert_eq!(r.cstr(15), Ok(&b"name"[..]));
        assert_eq!(r.u16(), Ok(9));
        assert_eq!(r.u16(), Err(Malformed));
        assert_eq!(Reader::new(b"no end").cstr(99), Err(Malformed));
        assert_eq!(Reader::new(b"toolong\0").cstr(3), Err(Malformed));
    }

    #[test]
    fn a_nul_cannot_split_a_string_in_two() {
        assert_eq!(Writer::new().cstr("a\0b").u16(1).finish(), [b'a', 0, 1, 0]);
    }

    #[test]
    fn tags_read_as_text_and_travel_reversed() {
        let tag = Reader::new(b"NB2W").u32().map(Tag).unwrap();
        assert_eq!(tag, product::WARCRAFT2);
        assert_eq!(tag.to_string(), "W2BN");
        assert_eq!(&tag.reversed(), b"NB2W");
    }
}
