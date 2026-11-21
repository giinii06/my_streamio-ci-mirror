//! Domain identifiers and input boundaries shared by the daemon and backend.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentId(String);

impl ContentId {
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        if raw.len() > 80
            || raw.is_empty()
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("content id must contain 1..80 ASCII alphanumeric or dash characters");
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(String);

impl SourceId {
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        if raw.len() > 80
            || raw.is_empty()
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("source id must contain 1..80 ASCII alphanumeric or dash characters");
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SwarmKey {
    V1([u8; 20]),
    V2([u8; 32]),
}

impl SwarmKey {
    pub fn as_hex(&self) -> String {
        let bytes: &[u8] = match self {
            Self::V1(value) => value,
            Self::V2(value) => value,
        };
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub fn parse_btih(hex: &str) -> Result<Self, &'static str> {
        let bytes = decode_hex::<20>(hex)?;
        Ok(Self::V1(bytes))
    }

    pub fn parse_btmh(hex: &str) -> Result<Self, &'static str> {
        let digest = hex
            .strip_prefix("1220")
            .ok_or("expected sha256 multihash prefix")?;
        let bytes = decode_hex::<32>(digest)?;
        Ok(Self::V2(bytes))
    }
}

fn decode_hex<const N: usize>(text: &str) -> Result<[u8; N], &'static str> {
    if text.len() != N * 2 {
        return Err("invalid hex length");
    }
    let mut bytes = [0u8; N];
    for (i, chunk) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let hex_digit = |value: u8| -> Option<u8> {
            match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'a'..=b'f' => Some(value - b'a' + 10),
                b'A'..=b'F' => Some(value - b'A' + 10),
                _ => None,
            }
        };
        let hi = hex_digit(chunk[0]).ok_or("invalid hex character")?;
        let lo = hex_digit(chunk[1]).ok_or("invalid hex character")?;
        bytes[i] = (hi << 4) | lo;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_ids() {
        assert!(ContentId::parse("../etc/passwd").is_err());
        assert!(SourceId::parse("  ").is_err());
        assert!(SourceId::parse("file_1").is_err());
        assert_eq!(ContentId::parse("test-22").unwrap().as_str(), "test-22");
    }

    #[test]
    fn validates_v1_and_v2_swarm_hash_lengths() {
        assert!(matches!(
            SwarmKey::parse_btih(&"a".repeat(40)),
            Ok(SwarmKey::V1(_))
        ));
        assert!(matches!(
            SwarmKey::parse_btmh(&format!("1220{}", "f".repeat(64))),
            Ok(SwarmKey::V2(_))
        ));
        assert!(SwarmKey::parse_btih(&"g".repeat(40)).is_err());
        assert!(SwarmKey::parse_btmh(&"f".repeat(64)).is_err());
    }
}
