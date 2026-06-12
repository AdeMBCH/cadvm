//! [`ObjectId`]: a dedicated, content-addressed identifier built on BLAKE3.
//!
//! The canonical textual form is `blake3:<hex>` where `<hex>` is the 64-character
//! lowercase hex digest. Using a dedicated type (instead of bare `String`s)
//! keeps hashes type-safe across the whole codebase.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The hash algorithm prefix used for every object id.
pub const ALGO_PREFIX: &str = "blake3";

/// A content-addressed object identifier (`blake3:<hex>`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    /// Lowercase hex digest (64 chars for BLAKE3-256).
    hex: String,
}

impl ObjectId {
    /// Build an id from the bytes of a BLAKE3 hash.
    pub fn from_hash(hash: &blake3::Hash) -> Self {
        ObjectId {
            hex: hash.to_hex().to_string(),
        }
    }

    /// Hash arbitrary bytes and return the resulting id.
    pub fn hash_bytes(bytes: &[u8]) -> Self {
        ObjectId::from_hash(&blake3::hash(bytes))
    }

    /// The lowercase hex digest, without the algorithm prefix.
    pub fn hex(&self) -> &str {
        &self.hex
    }

    /// The canonical `blake3:<hex>` string.
    pub fn canonical(&self) -> String {
        format!("{ALGO_PREFIX}:{}", self.hex)
    }

    /// A short, human-friendly prefix of the hex digest (default 8 chars).
    pub fn short(&self) -> &str {
        let n = self.hex.len().min(8);
        &self.hex[..n]
    }

    /// First two hex chars — used as the first storage shard directory.
    pub fn shard1(&self) -> &str {
        &self.hex[0..2]
    }

    /// Next two hex chars — used as the second storage shard directory.
    pub fn shard2(&self) -> &str {
        &self.hex[2..4]
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", ALGO_PREFIX, self.hex)
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({}:{})", ALGO_PREFIX, self.hex)
    }
}

/// Error returned when parsing a malformed [`ObjectId`].
#[derive(Debug, thiserror::Error)]
pub enum ObjectIdParseError {
    #[error("unexpected hash algorithm prefix: expected `{ALGO_PREFIX}:`, got `{0}`")]
    BadPrefix(String),
    #[error("hash hex digest has invalid length {0} (expected 64)")]
    BadLength(usize),
    #[error("hash hex digest contains non-hex characters")]
    NotHex,
}

impl FromStr for ObjectId {
    type Err = ObjectIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Accept both the canonical `blake3:<hex>` and a bare hex digest.
        let hex = match s.split_once(':') {
            Some((algo, rest)) => {
                if algo != ALGO_PREFIX {
                    return Err(ObjectIdParseError::BadPrefix(algo.to_string()));
                }
                rest
            }
            None => s,
        };
        if hex.len() != 64 {
            return Err(ObjectIdParseError::BadLength(hex.len()));
        }
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ObjectIdParseError::NotHex);
        }
        Ok(ObjectId {
            hex: hex.to_ascii_lowercase(),
        })
    }
}

impl Serialize for ObjectId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.canonical())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_canonical_form() {
        let id = ObjectId::hash_bytes(b"hello");
        let parsed: ObjectId = id.canonical().parse().unwrap();
        assert_eq!(id, parsed);
        assert_eq!(id.hex().len(), 64);
    }

    #[test]
    fn rejects_bad_prefix() {
        let err = "sha256:abcd".parse::<ObjectId>().unwrap_err();
        assert!(matches!(err, ObjectIdParseError::BadPrefix(_)));
    }
}
