//! A plain byte encoding, for what is signed, encrypted or backed up: a
//! number is 4 bytes big-endian, a byte string is its length then its
//! bytes, an optional one is a 0 byte or a 1 byte then the string.

/// Appends to a byte buffer.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u32(&mut self, number: u32) -> &mut Self {
        // `to_be_bytes`: the 4 bytes, most significant first.
        self.0.extend(number.to_be_bytes());
        // Returning `&mut Self` lets calls chain: `w.u32(1).bytes(b"x")`.
        self
    }

    pub fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        let length = u32::try_from(bytes.len()).expect("less than 4 GiB");
        self.u32(length);
        self.0.extend(bytes);
        self
    }

    pub fn text(&mut self, text: &str) -> &mut Self {
        self.bytes(text.as_bytes())
    }

    pub fn optional(&mut self, text: Option<&str>) -> &mut Self {
        match text {
            None => self.0.push(0),
            Some(text) => {
                self.0.push(1);
                self.text(text);
            }
        }
        self
    }
}

/// Reads what a Writer wrote; every method gives `None` past the end or on
/// malformed input, never panics.
// `'a`: the Reader borrows the bytes it reads and can't outlive them.
pub struct Reader<'a>(pub &'a [u8]);

impl<'a> Reader<'a> {
    pub fn u32(&mut self) -> Option<u32> {
        let (number, rest) = self.0.split_first_chunk::<4>()?;
        self.0 = rest;
        Some(u32::from_be_bytes(*number))
    }

    pub fn bytes(&mut self) -> Option<&'a [u8]> {
        let length = usize::try_from(self.u32()?).ok()?;
        // `split_at_checked` returns None if `length` is past the end.
        let (bytes, rest) = self.0.split_at_checked(length)?;
        self.0 = rest;
        Some(bytes)
    }

    pub fn text(&mut self) -> Option<String> {
        String::from_utf8(self.bytes()?.to_vec()).ok()
    }

    pub fn optional(&mut self) -> Option<Option<String>> {
        let (tag, rest) = self.0.split_first()?;
        self.0 = rest;
        match tag {
            0 => Some(None),
            1 => Some(Some(self.text()?)),
            _ => None,
        }
    }

    /// Whether everything was read: trailing bytes mean a malformed input.
    pub fn done(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_what_was_written() {
        let mut writer = Writer::default();
        writer
            .u32(7)
            .text("loyer")
            .optional(None)
            .optional(Some("é"));
        let mut reader = Reader(&writer.0);
        assert_eq!(reader.u32(), Some(7));
        assert_eq!(reader.text().as_deref(), Some("loyer"));
        assert_eq!(reader.optional(), Some(None));
        assert_eq!(reader.optional(), Some(Some("é".to_string())));
        assert!(reader.done());
        // Truncated input: `None`, no panic.
        let mut short = Reader(&writer.0[..6]);
        assert_eq!(short.u32(), Some(7));
        assert_eq!(short.text(), None);
    }
}
