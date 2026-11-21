//! Network-facing torrent adapter. The engine owns peer discovery and piece verification.
//! Applications must never treat arbitrary local file reads as torrent-verified bytes.

use anyhow::{anyhow, bail, Context, Result};
use librqbit::{api::TorrentIdOrHash, AddTorrent, Api, Session};
use my_streamio_domain::SwarmKey;
use serde_json::Value;
use std::{net::IpAddr, path::PathBuf, io::SeekFrom, time::Duration};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use url::Url;

pub const MAX_MAGNET_BYTES: usize = 8192;
const MAX_TRACKERS: usize = 32;

#[derive(Clone)]
pub struct TorrentBackend {
    api: Api,
}

impl TorrentBackend {
    pub async fn open(storage_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&storage_dir).context("creating torrent storage directory")?;
        let session = Session::new(storage_dir).await.context("starting torrent session")?;
        Ok(Self {
            api: Api::new(session, None),
        })
    }

    pub async fn add_magnet(&self, magnet: &str) -> Result<Value> {
        validate_magnet(magnet)?;
        let response = self
            .api
            .api_add_torrent(AddTorrent::from_url(magnet), None)
            .await
            .context("librqbit add_torrent failed")?;
        serde_json::to_value(response).context("serializing torrent response")
    }

    pub fn list_torrents(&self) -> Result<Value> {
        serde_json::to_value(self.api.api_torrent_list()).context("serializing torrent list")
    }

    /// Return file metadata without disclosing host storage paths.
    pub fn files(&self, torrent_id: usize) -> Result<Vec<PlayableFile>> {
        let details = self.api
            .api_torrent_details(TorrentIdOrHash::Id(torrent_id))
            .context("torrent not available")?;
        let files = details.files.context("torrent metadata unavailable")?;
        Ok(files.into_iter().enumerate().map(|(id, file)| PlayableFile {
            id,
            name: file.name,
            length: file.length,
            included: file.included,
        }).collect())
    }

    /// Read only bytes that librqbit has verified against the torrent pieces.
    /// Never open incomplete torrent files through std::fs::File.
    pub async fn read_verified_range(
        &self, torrent_id: usize, file_id: usize, offset: u64, count: usize,
    ) -> Result<Vec<u8>> {
        if count == 0 || count > MAX_HTTP_CHUNK {
            bail!("requested range exceeds cap");
        }
        let mut stream = self.api
            .api_stream(TorrentIdOrHash::Id(torrent_id), file_id)
            .await
            .context("cannot open torrent file stream")?;
        let read = async {
            stream.seek(SeekFrom::Start(offset)).await?;
            let mut bytes = vec![0; count];
            stream.read_exact(&mut bytes).await?;
            Ok::<Vec<u8>, std::io::Error>(bytes)
        };
        tokio::time::timeout(Duration::from_secs(20), read)
            .await
            .context("timed out waiting for verified pieces")?
            .context("torrent stream ended early")
    }
}

pub const MAX_HTTP_CHUNK: usize = 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlayableFile {
    pub id: usize,
    pub name: String,
    pub length: u64,
    pub included: bool,
}

/// Validates a magnet before letting the native torrent engine contact any tracker.
/// DNS-rebinding / remote tracker policy will require additional defenses before public use.
pub fn validate_magnet(raw: &str) -> Result<SwarmKey> {
    if raw.len() > MAX_MAGNET_BYTES {
        bail!("magnet exceeds maximum length");
    }

    let url = Url::parse(raw).context("invalid magnet URI")?;
    if url.scheme() != "magnet" {
        bail!("only magnet URI input is accepted");
    }
    let mut identity = None;
    let mut trackers = 0usize;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "xt" => {
                let value = value.as_ref();
                let parsed = if let Some(hash) = value.strip_prefix("urn:btih:") {
                    Some(parse_btih(hash)?)
                } else if let Some(hash) = value.strip_prefix("urn:btmh:") {
                    Some(SwarmKey::parse_btmh(hash).map_err(|e| anyhow!(e))?)
                } else {
                    None
                };
                if let Some(next) = parsed {
                    if identity.as_ref().is_some_and(|current| current != &next) {
                        bail!("magnet includes conflicting infohash values");
                    }
                    identity = Some(next);
                }
            }
            "tr" => {
                trackers += 1;
                if trackers > MAX_TRACKERS {
                    bail!("too many tracker URLs");
                }
                validate_tracker(&value)?;
            }
            _ => {}
        }
    }

    identity.ok_or_else(|| anyhow!("missing supported xt=urn:btih or urn:btmh"))
}

fn validate_tracker(raw: &str) -> Result<()> {
    let tracker = Url::parse(raw).context("invalid tracker URL")?;
    if !matches!(tracker.scheme(), "http" | "https" | "udp") {
        bail!("unsupported tracker protocol");
    }
    let host = tracker.host_str().ok_or_else(|| anyhow!("tracker has no host"))?;
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        bail!("local tracker not allowed by default");
    }
    if host.parse::<IpAddr>().is_ok() {
        bail!("tracker IP literals are disabled by default");
    }
    Ok(())
}

fn parse_btih(value: &str) -> Result<SwarmKey> {
    if value.len() == 40 {
        return SwarmKey::parse_btih(value).map_err(|e| anyhow!(e));
    }
    if value.len() != 32 {
        bail!("btih hash must be 40 hex or 32 base32 characters");
    }

    let mut decoded = [0u8; 20];
    let mut bit_buffer = 0u32;
    let mut remaining = 0u8;
    let mut written = 0usize;
    for byte in value.bytes() {
        let upper = byte.to_ascii_uppercase();
        let digit = match upper {
            b'A'..=b'Z' => upper - b'A',
            b'2'..=b'7' => upper - b'2' + 26,
            _ => bail!("invalid base32 character"),
        };
        bit_buffer = (bit_buffer << 5) | u32::from(digit);
        remaining += 5;
        if remaining >= 8 {
            remaining -= 8;
            decoded[written] = ((bit_buffer >> remaining) & 0xff) as u8;
            written += 1;
        }
    }
    if written != 20 || remaining != 0 {
        bail!("incorrect base32 hash length");
    }
    Ok(SwarmKey::V1(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnet_hex_and_base32_are_same_identity() {
        let a = validate_magnet(&format!("magnet:?xt=urn:btih:{}", "00".repeat(20))).unwrap();
        let b = validate_magnet(&format!("magnet:?xt=urn:btih:{}", "A".repeat(32))).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_conflicting_identity_and_bad_trackers() {
        let magnet = format!(
            "magnet:?xt=urn:btih:{}&xt=urn:btih:{}",
            "aa".repeat(20),
            "bb".repeat(20)
        );
        assert!(validate_magnet(&magnet).is_err());
        let magnet = format!(
            "magnet:?xt=urn:btih:{}&tr=http%3A%2F%2F127.0.0.1%3A8080",
            "aa".repeat(20)
        );
        assert!(validate_magnet(&magnet).is_err());
    }

    #[test]
    fn rejects_malformed_and_oversized_magnets() {
        assert!(validate_magnet("https://example.com/").is_err());
        assert!(validate_magnet("magnet:?dn=movie").is_err());
        assert!(validate_magnet(&"a".repeat(MAX_MAGNET_BYTES + 1)).is_err());
    }

    #[test]
    fn preserves_v2_hash_identity() {
        let hash = format!("1220{}", "ac".repeat(32));
        let result = validate_magnet(&format!("magnet:?xt=urn:btmh:{hash}")).unwrap();
        assert!(matches!(result, SwarmKey::V2(_)));
    }
}
