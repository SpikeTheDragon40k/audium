//! Typed ids. Database ids are stable SQLite row ids; [`BookUid`] is the
//! identity of a book that survives rescans, moves and renames.

use std::fmt;

macro_rules! row_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub i64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

row_id!(LibraryId);
row_id!(BookId);
row_id!(TrackId);

/// Stable, random book identity (a UUID in its hyphenated text form).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BookUid(String);

impl BookUid {
    /// Wraps an existing uid. Returns `None` unless it is a hyphenated UUID.
    pub fn parse(text: &str) -> Option<Self> {
        let valid = text.len() == 36
            && text.char_indices().all(|(i, c)| match i {
                8 | 13 | 18 | 23 => c == '-',
                _ => c.is_ascii_hexdigit(),
            });
        valid.then(|| BookUid(text.to_ascii_lowercase()))
    }

    /// Builds a version-4 UUID from 16 random bytes.
    pub fn from_random_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        BookUid(format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BookUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_uid_is_valid_v4() {
        let uid = BookUid::from_random_bytes([0xff; 16]);
        assert_eq!(uid.as_str().len(), 36);
        assert_eq!(uid.as_str().as_bytes()[14], b'4');
        assert!(BookUid::parse(uid.as_str()).is_some());
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(BookUid::parse("").is_none());
        assert!(BookUid::parse("not-a-uuid").is_none());
        assert!(BookUid::parse("zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz").is_none());
        assert!(BookUid::parse("123e4567-e89b-42d3-a456-426614174000").is_some());
    }
}
